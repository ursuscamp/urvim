//! Notification primitives for user-facing runtime messages.
//!
//! This module provides queue-backed notification state, top-right floating
//! popup rendering, bottom-right LSP progress rendering, and helper APIs used
//! by notification macros.

use crate::globals;
use crate::lsp::runtime::LspServerStatus;
use crate::screen::Screen;
use crate::terminal::{Color, Style};
use crate::ui::floating_window::{FloatingAnchor, FloatingWindowFrame};
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use crate::window::{Position, Size};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const NORMAL_TTL: Duration = Duration::from_secs(3);
const BACKLOG_TTL: Duration = Duration::from_secs(1);
const MAX_POPUP_CONTENT_WIDTH: usize = 48;
const MAX_PROGRESS_CONTENT_WIDTH: usize = 48;

/// Notification severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Informational notification.
    Info,
    /// Warning notification.
    Warn,
    /// Error notification.
    Error,
}

/// A notification currently shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationMessage {
    /// Notification severity.
    pub level: NotificationLevel,
    /// Rendered notification text.
    pub text: String,
    /// Message creation timestamp.
    pub created_at: Instant,
    /// Timestamp at which the message expires.
    pub expires_at: Instant,
}

impl NotificationMessage {
    /// Creates a notification message with an explicit TTL.
    pub fn new(level: NotificationLevel, text: String, created_at: Instant, ttl: Duration) -> Self {
        Self {
            level,
            text,
            created_at,
            expires_at: created_at + ttl,
        }
    }

    /// Returns true when this message is expired at `now`.
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.expires_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingNotification {
    level: NotificationLevel,
    text: String,
    created_at: Instant,
    backlog_ttl: bool,
}

/// In-memory notification queue state.
#[derive(Debug, Default)]
pub struct NotificationState {
    active: Option<NotificationMessage>,
    pending: VecDeque<PendingNotification>,
    redraw_requested: bool,
}

impl NotificationState {
    /// Creates an empty notification queue state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the active notification, if one is currently displayed.
    pub fn active(&self) -> Option<&NotificationMessage> {
        self.active.as_ref()
    }

    /// Returns the number of queued pending notifications.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Enqueues a notification message.
    ///
    /// Empty or whitespace-only messages are ignored.
    pub fn enqueue(&mut self, level: NotificationLevel, text: String, now: Instant) -> bool {
        if text.trim().is_empty() {
            return false;
        }

        if self.active.is_none() {
            self.active = Some(NotificationMessage::new(level, text, now, NORMAL_TTL));
            self.redraw_requested = true;
            return true;
        }

        let pending = PendingNotification {
            level,
            text,
            created_at: now,
            backlog_ttl: true,
        };

        self.pending.push_back(pending);
        self.redraw_requested = true;
        true
    }

    /// Prunes expired active notifications and advances the queue.
    pub fn prune_and_advance(&mut self, now: Instant) -> bool {
        let mut changed = false;

        while self
            .active
            .as_ref()
            .is_some_and(|active| active.expired(now))
        {
            changed = true;
            if let Some(next) = self.pending.pop_front() {
                let ttl = if next.backlog_ttl {
                    BACKLOG_TTL
                } else {
                    NORMAL_TTL
                };
                self.active = Some(NotificationMessage::new(next.level, next.text, now, ttl));
            } else {
                self.active = None;
            }
        }

        if changed {
            self.redraw_requested = true;
        }

        changed
    }

    /// Clears active and pending notifications.
    pub fn clear(&mut self) {
        self.active = None;
        self.pending.clear();
        self.redraw_requested = false;
    }

    /// Requests a redraw for the notification surface.
    pub fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    /// Returns and clears the redraw-requested flag.
    pub fn take_redraw_requested(&mut self) -> bool {
        let requested = self.redraw_requested;
        self.redraw_requested = false;
        requested
    }
}

/// Logs and enqueues a user-facing notification.
pub fn notify_message(level: NotificationLevel, message: String) {
    globals::enqueue_notification(level, message);
}

/// Renders the currently active notification as a top-right floating popup.
pub fn render_active_banner(screen: &mut Screen, origin: Position, size: Size, now: Instant) {
    if size.rows < 3 || size.cols < 3 {
        return;
    }

    let message = globals::active_notification(now);
    let Some(message) = message else {
        return;
    };

    render_notification_popup(screen, origin, size, &message);
}

/// Renders the current LSP progress as a bottom-right floating popup.
pub fn render_active_progress_banner(screen: &mut Screen, origin: Position, size: Size) {
    if size.rows < 3 || size.cols < 3 {
        return;
    }

    let statuses = globals::with_lsp_runtime(|runtime| {
        runtime
            .map(|runtime| runtime.server_statuses())
            .unwrap_or_default()
    });
    if statuses.is_empty() {
        return;
    }

    render_progress_popup(screen, origin, size, statuses.as_slice());
}

/// Floating widget that renders LSP server progress.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProgressWidget;

impl ProgressWidget {
    /// Creates a progress widget.
    pub fn new() -> Self {
        Self
    }
}

impl Widget for ProgressWidget {
    fn handle_ui_event(&mut self, _event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        UiEventResult::NotHandled
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        render_active_progress_banner(screen, rect.origin, rect.size);
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Passive
    }
}

fn render_notification_popup(
    screen: &mut Screen,
    origin: Position,
    size: Size,
    message: &NotificationMessage,
) {
    let available_content_cols = usize::from(size.cols.saturating_sub(2));
    let available_content_rows = usize::from(size.rows.saturating_sub(2));
    if available_content_cols == 0 || available_content_rows == 0 {
        return;
    }

    let wrap_width = available_content_cols.min(MAX_POPUP_CONTENT_WIDTH);
    let wrapped_lines = wrap_notification_text(message.text.as_str(), wrap_width);
    if wrapped_lines.is_empty() {
        return;
    }

    let rendered_lines = wrapped_lines
        .into_iter()
        .take(available_content_rows)
        .collect::<Vec<_>>();
    if rendered_lines.is_empty() {
        return;
    }

    let content_width = rendered_lines
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0)
        .min(available_content_cols);
    if content_width == 0 {
        return;
    }

    let popup_width = content_width + 2;
    let popup_height = rendered_lines.len() + 2;
    if popup_width < 3 || popup_height < 3 {
        return;
    }

    let popup_width = popup_width.min(usize::from(size.cols)) as u16;
    let popup_height = popup_height.min(usize::from(size.rows)) as u16;
    if popup_width < 3 || popup_height < 3 {
        return;
    }

    let frame = FloatingWindowFrame::resolve(
        origin,
        size,
        popup_height.saturating_sub(2),
        popup_width.saturating_sub(2),
        FloatingAnchor::TopRight,
    );
    let Some(frame) = frame else {
        return;
    };

    let border_style = level_style(message.level);
    let body_style = notification_body_style();
    frame.render_bordered(screen, border_style, body_style);

    for (line_idx, line) in rendered_lines.iter().enumerate() {
        let row = frame.content_origin.row + line_idx as u16;
        if row >= frame.content_origin.row + frame.content_size.rows {
            break;
        }
        screen.write_string(row, frame.content_origin.col, border_style, line.as_str());
    }
}

fn render_progress_popup(
    screen: &mut Screen,
    origin: Position,
    size: Size,
    statuses: &[LspServerStatus],
) {
    let available_content_cols = usize::from(size.cols.saturating_sub(2));
    let available_content_rows = usize::from(size.rows.saturating_sub(2));
    if available_content_cols == 0 || available_content_rows == 0 {
        return;
    }

    let wrap_width = available_content_cols.min(MAX_PROGRESS_CONTENT_WIDTH);
    let mut rendered_lines = Vec::new();
    for status in statuses {
        let status_text = format!("{}: {}", status.server_name, status.message);
        rendered_lines.extend(wrap_notification_text(status_text.as_str(), wrap_width));
    }

    rendered_lines.truncate(available_content_rows);
    if rendered_lines.is_empty() {
        return;
    }

    let content_width = rendered_lines
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0)
        .min(available_content_cols);
    if content_width == 0 {
        return;
    }

    let popup_width = content_width + 2;
    let popup_height = rendered_lines.len() + 2;
    if popup_width < 3 || popup_height < 3 {
        return;
    }

    let popup_width = popup_width.min(usize::from(size.cols)) as u16;
    let popup_height = popup_height.min(usize::from(size.rows)) as u16;
    if popup_width < 3 || popup_height < 3 {
        return;
    }

    let frame = FloatingWindowFrame::resolve(
        origin,
        size,
        popup_height.saturating_sub(2),
        popup_width.saturating_sub(2),
        FloatingAnchor::BottomRight,
    );
    let Some(frame) = frame else {
        return;
    };

    let border_style = progress_border_style();
    let body_style = progress_body_style();
    frame.render_bordered(screen, border_style, body_style);

    for (line_idx, line) in rendered_lines.iter().enumerate() {
        let row = frame.content_origin.row + line_idx as u16;
        if row >= frame.content_origin.row + frame.content_size.rows {
            break;
        }
        screen.write_string(row, frame.content_origin.col, border_style, line.as_str());
    }
}

fn level_style(level: NotificationLevel) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| match level {
                NotificationLevel::Info => theme.resolve_name_with_default("ui.notification.info"),
                NotificationLevel::Warn => theme.resolve_name_with_default("ui.notification.warn"),
                NotificationLevel::Error => {
                    theme.resolve_name_with_default("ui.notification.error")
                }
            })
            .unwrap_or_else(|| match level {
                NotificationLevel::Info => Style::new().fg(Color::ansi(75)),
                NotificationLevel::Warn => Style::new().fg(Color::ansi(221)).bold(),
                NotificationLevel::Error => Style::new().fg(Color::ansi(203)).bold(),
            })
    })
}

fn notification_body_style() -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default("ui.window"))
            .unwrap_or_default()
    })
}

fn progress_border_style() -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default("ui.window.lines.border"))
            .unwrap_or_default()
    })
}

fn progress_body_style() -> Style {
    notification_body_style()
}

fn wrap_notification_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            result.push(String::new());
            continue;
        }

        let graphemes = paragraph
            .grapheme_indices(true)
            .map(|(start_byte, grapheme)| GraphemeSlice {
                start_byte,
                width: UnicodeWidthStr::width(grapheme),
                is_whitespace: grapheme.chars().all(char::is_whitespace),
            })
            .collect::<Vec<_>>();

        if graphemes.is_empty() {
            result.push(String::new());
            continue;
        }

        let mut start = 0usize;
        while start < graphemes.len() {
            let mut width = 0usize;
            let mut end = start;
            let mut last_soft_break = None;

            while end < graphemes.len() {
                let grapheme = graphemes[end];
                let next_width = width + grapheme.width;
                if next_width > max_width {
                    if end == start {
                        end += 1;
                    }
                    break;
                }

                width = next_width;
                end += 1;

                if end < graphemes.len()
                    && graphemes[end - 1].is_whitespace != graphemes[end].is_whitespace
                {
                    last_soft_break = Some(end);
                }
            }

            let segment_end = if end < graphemes.len() {
                last_soft_break
                    .filter(|break_idx| *break_idx > start)
                    .unwrap_or(end)
            } else {
                graphemes.len()
            };

            let start_byte = graphemes[start].start_byte;
            let end_byte = if segment_end < graphemes.len() {
                graphemes[segment_end].start_byte
            } else {
                paragraph.len()
            };
            result.push(paragraph[start_byte..end_byte].to_string());
            start = segment_end;
        }
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

#[derive(Debug, Clone, Copy)]
struct GraphemeSlice {
    start_byte: usize,
    width: usize,
    is_whitespace: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::screen::Screen;
    use crate::window::{Position, Size};

    #[test]
    fn enqueue_sets_active_message() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "Saved".to_string(), now));
        let active = state.active().expect("active notification");
        assert_eq!(active.text, "Saved");
        assert_eq!(active.level, NotificationLevel::Info);
        assert!(active.expires_at.duration_since(active.created_at) >= NORMAL_TTL);
    }

    #[test]
    fn enqueue_multiple_messages_keeps_fifo_pending() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Error, "three".to_string(), now));

        assert_eq!(state.pending_len(), 2);
        assert_eq!(
            state.active().map(|message| message.text.as_str()),
            Some("one")
        );
    }

    #[test]
    fn prune_advances_queue_in_order() {
        let mut state = NotificationState::new();
        let now = Instant::now();
        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));

        let first_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(first_expiry));
        assert_eq!(
            state.active().map(|message| message.text.as_str()),
            Some("two")
        );

        let second_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(second_expiry));
        assert!(state.active().is_none());
    }

    #[test]
    fn backlog_messages_use_shorter_ttl() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Error, "three".to_string(), now));

        let first_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(first_expiry));

        let second = state.active().expect("active");
        assert_eq!(
            second.expires_at.duration_since(second.created_at),
            BACKLOG_TTL
        );
    }

    #[test]
    fn empty_messages_are_ignored() {
        let mut state = NotificationState::new();

        assert!(!state.enqueue(NotificationLevel::Info, "   ".to_string(), Instant::now()));
        assert!(state.active().is_none());
    }

    #[test]
    fn backlog_messages_keep_short_ttl_until_queue_clears() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));

        let first_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(first_expiry));
        let second = state.active().expect("active");
        assert_eq!(
            second.expires_at.duration_since(second.created_at),
            BACKLOG_TTL
        );

        let second_expiry = second.expires_at;
        assert!(state.prune_and_advance(second_expiry));
        assert!(state.active().is_none());

        let after_clear = second_expiry + Duration::from_millis(10);
        assert!(state.enqueue(NotificationLevel::Info, "three".to_string(), after_clear));
        let third = state.active().expect("active");
        assert_eq!(
            third.expires_at.duration_since(third.created_at),
            NORMAL_TTL
        );
    }

    #[test]
    fn wrap_notification_text_prefers_whitespace_boundaries() {
        assert_eq!(
            wrap_notification_text("hello world", 6),
            vec!["hello ", "world"]
        );
    }

    #[test]
    fn wrap_notification_text_preserves_grapheme_boundaries() {
        assert_eq!(wrap_notification_text("éclair", 2), vec!["éc", "la", "ir"]);
        assert_eq!(wrap_notification_text("😀ab", 2), vec!["😀", "ab"]);
    }

    #[test]
    fn render_active_banner_places_popup_top_right() {
        let _config_guard = globals::set_test_config(Config::default());
        let message = NotificationMessage::new(
            NotificationLevel::Info,
            "ok".to_string(),
            Instant::now(),
            Duration::from_secs(3),
        );
        let mut screen = Screen::new(4, 8);
        render_notification_popup(&mut screen, Position::new(0, 0), Size::new(4, 8), &message);

        assert_eq!(
            screen.get_cell_mut(0, 4).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(0, 7).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(1, 4).map(|cell| cell.text.clone()),
            Some("|".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(1, 7).map(|cell| cell.text.clone()),
            Some("|".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(1, 5).map(|cell| cell.text.clone()),
            Some("o".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(1, 6).map(|cell| cell.text.clone()),
            Some("k".to_string())
        );
    }

    #[test]
    fn render_active_progress_banner_places_popup_bottom_right() {
        let _config_guard = globals::set_test_config(Config::default());
        let statuses = vec![LspServerStatus {
            server_name: "lsp".to_string(),
            message: "ok".to_string(),
        }];
        let mut screen = Screen::new(4, 12);
        render_progress_popup(
            &mut screen,
            Position::new(0, 0),
            Size::new(4, 12),
            &statuses,
        );

        assert_eq!(
            screen.get_cell_mut(1, 3).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(1, 11).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(2, 4).map(|cell| cell.text.clone()),
            Some("l".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(2, 10).map(|cell| cell.text.clone()),
            Some("k".to_string())
        );
    }

    #[test]
    fn render_active_banner_wraps_multiline_text_in_popup() {
        let _config_guard = globals::set_test_config(Config::default());
        let message = NotificationMessage::new(
            NotificationLevel::Warn,
            "hello world".to_string(),
            Instant::now(),
            Duration::from_secs(3),
        );
        let mut screen = Screen::new(5, 12);
        render_notification_popup(&mut screen, Position::new(0, 0), Size::new(5, 12), &message);

        assert_eq!(
            screen.get_cell_mut(1, 5).map(|cell| cell.text.clone()),
            Some("h".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(1, 10).map(|cell| cell.text.clone()),
            Some(" ".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(2, 5).map(|cell| cell.text.clone()),
            Some("w".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(3, 4).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
    }

    #[test]
    fn render_active_banner_uses_distinct_styles_per_level() {
        assert_ne!(
            level_style(NotificationLevel::Info),
            level_style(NotificationLevel::Warn)
        );
        assert_ne!(
            level_style(NotificationLevel::Warn),
            level_style(NotificationLevel::Error)
        );
    }
}
