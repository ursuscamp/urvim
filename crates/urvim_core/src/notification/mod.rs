//! Notification primitives for user-facing runtime messages.
//!
//! This module provides queue-backed notification state, top-right floating
//! popup rendering, bottom-right LSP progress rendering, and helper APIs used
//! by notification macros.

use crate::globals;
use crate::lsp::runtime::LspServerStatus;
use crate::screen::Screen;
use crate::ui::geometry::{Position, Size};
use crate::ui::overlay::frame::{OverlayAnchor, OverlayFrame, OverlayMargins, OverlayPlacement};
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use urvim_terminal::{Color, Style};

const NORMAL_TTL: Duration = Duration::from_secs(3);
const MAX_VISIBLE_NOTIFICATIONS: usize = 3;
const STACK_GAP: u16 = 1;
const MIN_POPUP_HEIGHT: u16 = 3;
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
}

/// In-memory notification queue state.
#[derive(Debug, Default)]
pub struct NotificationState {
    visible: VecDeque<NotificationMessage>,
    pending: VecDeque<PendingNotification>,
    redraw_requested: bool,
}

impl NotificationState {
    /// Creates an empty notification queue state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns visible notifications from newest to oldest.
    pub fn visible(&self) -> &VecDeque<NotificationMessage> {
        &self.visible
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

        if self.visible.len() < MAX_VISIBLE_NOTIFICATIONS {
            self.visible
                .push_front(NotificationMessage::new(level, text, now, NORMAL_TTL));
            self.redraw_requested = true;
            return true;
        }

        let pending = PendingNotification { level, text };

        self.pending.push_back(pending);
        self.redraw_requested = true;
        true
    }

    /// Prunes expired visible notifications and fills newly available slots.
    pub fn prune_and_advance(&mut self, now: Instant) -> bool {
        let previous_visible_len = self.visible.len();
        self.visible
            .retain(|notification| !notification.expired(now));
        let mut changed = self.visible.len() != previous_visible_len;

        while self.visible.len() < MAX_VISIBLE_NOTIFICATIONS {
            let Some(next) = self.pending.pop_front() else {
                break;
            };

            // Pending notifications become the newest visible entries when a
            // slot opens. This preserves newest-first ordering even when more
            // than one slot becomes available at the same time.
            self.visible.push_front(NotificationMessage::new(
                next.level, next.text, now, NORMAL_TTL,
            ));
            changed = true;
        }

        if changed {
            self.redraw_requested = true;
        }

        changed
    }

    /// Clears visible and pending notifications.
    pub fn clear(&mut self) {
        self.visible.clear();
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

/// Renders visible notifications as a newest-first top-right floating stack.
pub fn render_active_banner(screen: &mut Screen, origin: Position, size: Size, now: Instant) {
    if size.rows < 3 || size.cols < 3 {
        return;
    }

    let messages = globals::visible_notifications(now);
    if messages.is_empty() {
        return;
    }

    let progress_height = active_progress_popup_height(size);
    render_notification_stack(screen, origin, size, messages.as_slice(), progress_height);
}

fn render_notification_stack(
    screen: &mut Screen,
    origin: Position,
    size: Size,
    messages: &[NotificationMessage],
    progress_height: u16,
) {
    let progress_gap = (progress_height > 0).then_some(STACK_GAP).unwrap_or(0);
    let available_rows = size
        .rows
        .saturating_sub(progress_height)
        .saturating_sub(progress_gap);
    let visible_count = messages
        .len()
        .min(MAX_VISIBLE_NOTIFICATIONS)
        .min(stack_capacity(available_rows));
    if visible_count == 0 {
        return;
    }

    let mut row_offset = 0u16;
    for (index, message) in messages.iter().take(visible_count).enumerate() {
        let remaining_rows = available_rows.saturating_sub(row_offset);
        let remaining_popups = (visible_count - index) as u16;
        let remaining_gaps = STACK_GAP.saturating_mul(remaining_popups.saturating_sub(1));
        let max_popup_height = remaining_rows
            .saturating_sub(remaining_gaps)
            .checked_div(remaining_popups)
            .unwrap_or(0);
        if max_popup_height < MIN_POPUP_HEIGHT {
            break;
        }

        let popup_size = Size::new(max_popup_height, size.cols);
        let popup_origin = Position::new(origin.row.saturating_add(row_offset), origin.col);
        let Some(popup_height) =
            render_notification_popup(screen, popup_origin, popup_size, message)
        else {
            break;
        };
        row_offset = row_offset.saturating_add(popup_height.saturating_add(STACK_GAP));
    }
}

fn stack_capacity(available_rows: u16) -> usize {
    if available_rows < MIN_POPUP_HEIGHT {
        return 0;
    }

    usize::from(
        available_rows
            .saturating_add(STACK_GAP)
            .checked_div(MIN_POPUP_HEIGHT.saturating_add(STACK_GAP))
            .unwrap_or(0),
    )
    .min(MAX_VISIBLE_NOTIFICATIONS)
}

fn active_progress_popup_height(size: Size) -> u16 {
    let statuses = globals::with_lsp_runtime(|runtime| {
        runtime
            .map(|runtime| runtime.server_statuses())
            .unwrap_or_default()
    });
    progress_popup_height(size, statuses.as_slice())
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
) -> Option<u16> {
    let available_content_cols = usize::from(size.cols.saturating_sub(2));
    let available_content_rows = usize::from(size.rows.saturating_sub(2));
    if available_content_cols == 0 || available_content_rows == 0 {
        return None;
    }

    let wrap_width = available_content_cols.min(MAX_POPUP_CONTENT_WIDTH);
    let wrapped_lines = wrap_notification_text(message.text.as_str(), wrap_width);
    if wrapped_lines.is_empty() {
        return None;
    }

    let rendered_lines = wrapped_lines
        .into_iter()
        .take(available_content_rows)
        .collect::<Vec<_>>();
    if rendered_lines.is_empty() {
        return None;
    }

    let content_width = rendered_lines
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0)
        .min(available_content_cols);
    if content_width == 0 {
        return None;
    }

    let popup_width = content_width + 2;
    let popup_height = rendered_lines.len() + 2;
    if popup_width < 3 || popup_height < 3 {
        return None;
    }

    let popup_width = popup_width.min(usize::from(size.cols)) as u16;
    let popup_height = popup_height.min(usize::from(size.rows)) as u16;
    if popup_width < 3 || popup_height < 3 {
        return None;
    }

    let frame = OverlayFrame::resolve_placement(
        origin,
        size,
        popup_height.saturating_sub(2),
        popup_width.saturating_sub(2),
        OverlayPlacement::Anchored {
            anchor: OverlayAnchor::TopRight,
            margins: OverlayMargins::default(),
        },
    );
    let Some(frame) = frame else {
        return None;
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

    Some(frame.size.rows)
}

fn render_progress_popup(
    screen: &mut Screen,
    origin: Position,
    size: Size,
    statuses: &[LspServerStatus],
) {
    let rendered_lines = progress_rendered_lines(size, statuses);
    if rendered_lines.is_empty() {
        return;
    }

    let available_content_cols = usize::from(size.cols.saturating_sub(2));

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

    let frame = OverlayFrame::resolve_placement(
        origin,
        size,
        popup_height.saturating_sub(2),
        popup_width.saturating_sub(2),
        OverlayPlacement::Anchored {
            anchor: OverlayAnchor::BottomRight,
            margins: OverlayMargins::default(),
        },
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

fn progress_popup_height(size: Size, statuses: &[LspServerStatus]) -> u16 {
    let rendered_lines = progress_rendered_lines(size, statuses);
    if rendered_lines.is_empty() {
        0
    } else {
        rendered_lines
            .len()
            .saturating_add(2)
            .min(usize::from(size.rows)) as u16
    }
}

fn progress_rendered_lines(size: Size, statuses: &[LspServerStatus]) -> Vec<String> {
    let available_content_cols = usize::from(size.cols.saturating_sub(2));
    let available_content_rows = usize::from(size.rows.saturating_sub(2));
    if available_content_cols == 0 || available_content_rows == 0 {
        return Vec::new();
    }

    let wrap_width = available_content_cols.min(MAX_PROGRESS_CONTENT_WIDTH);
    let mut rendered_lines = Vec::new();
    for status in statuses {
        let status_text = format!("{}: {}", status.server_name, status.message);
        rendered_lines.extend(wrap_notification_text(status_text.as_str(), wrap_width));
    }
    rendered_lines.truncate(available_content_rows);
    rendered_lines
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
    use crate::ui::geometry::{Position, Size};

    #[test]
    fn enqueue_sets_visible_message_with_normal_ttl() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "Saved".to_string(), now));
        let visible = state.visible();
        assert_eq!(
            visible.front().map(|message| message.text.as_str()),
            Some("Saved")
        );
        assert_eq!(
            visible.front().map(|message| message.level),
            Some(NotificationLevel::Info)
        );
        assert_eq!(
            visible
                .front()
                .expect("visible notification")
                .expires_at
                .duration_since(now),
            NORMAL_TTL
        );
    }

    #[test]
    fn enqueue_multiple_messages_stacks_newest_first_and_keeps_fifo_pending() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Error, "three".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Info, "four".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "five".to_string(), now));

        assert_eq!(
            state
                .visible()
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            vec!["three", "two", "one"]
        );
        assert_eq!(state.pending_len(), 2);
    }

    #[test]
    fn prune_removes_expired_entries_independently_and_promotes_pending() {
        let mut state = NotificationState::new();
        let now = Instant::now();
        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(
            NotificationLevel::Warn,
            "two".to_string(),
            now + Duration::from_secs(1),
        ));
        assert!(state.enqueue(
            NotificationLevel::Error,
            "three".to_string(),
            now + Duration::from_secs(2),
        ));
        assert!(state.enqueue(
            NotificationLevel::Info,
            "four".to_string(),
            now + Duration::from_secs(3),
        ));
        assert!(state.enqueue(
            NotificationLevel::Warn,
            "five".to_string(),
            now + Duration::from_secs(4),
        ));

        assert!(state.prune_and_advance(now + Duration::from_secs(3)));
        assert_eq!(
            state
                .visible()
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            vec!["four", "three", "two"]
        );
        assert_eq!(state.pending_len(), 1);

        assert!(state.prune_and_advance(now + Duration::from_secs(4)));
        assert_eq!(
            state
                .visible()
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            vec!["five", "four", "three"]
        );
        assert_eq!(
            state
                .visible()
                .front()
                .expect("promoted notification")
                .expires_at,
            now + Duration::from_secs(7)
        );
    }

    #[test]
    fn empty_messages_are_ignored() {
        let mut state = NotificationState::new();

        assert!(!state.enqueue(NotificationLevel::Info, "   ".to_string(), Instant::now()));
        assert!(state.visible().is_empty());
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
        assert!(
            render_notification_popup(&mut screen, Position::new(0, 0), Size::new(4, 8), &message)
                .is_some()
        );

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
    fn render_notification_stack_places_popups_newest_first_without_overlap() {
        let _config_guard = globals::set_test_config(Config::default());
        let now = Instant::now();
        let messages = vec![
            NotificationMessage::new(
                NotificationLevel::Error,
                "three".to_string(),
                now,
                NORMAL_TTL,
            ),
            NotificationMessage::new(NotificationLevel::Warn, "two".to_string(), now, NORMAL_TTL),
            NotificationMessage::new(NotificationLevel::Info, "one".to_string(), now, NORMAL_TTL),
        ];
        let mut screen = Screen::new(15, 12);

        render_notification_stack(
            &mut screen,
            Position::new(0, 0),
            Size::new(15, 12),
            messages.as_slice(),
            0,
        );

        assert_eq!(
            screen.get_cell_mut(0, 5).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(4, 7).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(8, 7).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(1, 6).map(|cell| cell.text.clone()),
            Some("t".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(5, 8).map(|cell| cell.text.clone()),
            Some("t".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(9, 8).map(|cell| cell.text.clone()),
            Some("o".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(3, 5).map(|cell| cell.text.clone()),
            Some(" ".to_string())
        );
    }

    #[test]
    fn notification_stack_reserves_progress_popup_space() {
        let _config_guard = globals::set_test_config(Config::default());
        let now = Instant::now();
        let messages = vec![
            NotificationMessage::new(
                NotificationLevel::Error,
                "three".to_string(),
                now,
                NORMAL_TTL,
            ),
            NotificationMessage::new(NotificationLevel::Warn, "two".to_string(), now, NORMAL_TTL),
            NotificationMessage::new(NotificationLevel::Info, "one".to_string(), now, NORMAL_TTL),
        ];
        let mut screen = Screen::new(15, 12);

        render_notification_stack(
            &mut screen,
            Position::new(0, 0),
            Size::new(15, 12),
            messages.as_slice(),
            3,
        );

        assert_eq!(
            screen.get_cell_mut(8, 7).map(|cell| cell.text.clone()),
            Some("+".to_string())
        );
        assert_eq!(
            screen.get_cell_mut(12, 7).map(|cell| cell.text.clone()),
            Some(" ".to_string())
        );
    }

    #[test]
    fn stack_capacity_reduces_visible_count_for_small_regions() {
        assert_eq!(stack_capacity(2), 0);
        assert_eq!(stack_capacity(3), 1);
        assert_eq!(stack_capacity(7), 2);
        assert_eq!(stack_capacity(11), 3);
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
        assert!(
            render_notification_popup(&mut screen, Position::new(0, 0), Size::new(5, 12), &message)
                .is_some()
        );

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
