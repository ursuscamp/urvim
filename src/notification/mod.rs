//! Notification primitives for user-facing runtime messages.
//!
//! This module provides queue-backed notification state, top-right floating
//! popup rendering, and helper APIs used by notification macros.

use crate::globals;
use crate::screen::Screen;
use crate::terminal::{Color, Style};
use crate::window::{Position, Size};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const NORMAL_TTL: Duration = Duration::from_secs(3);
const BACKLOG_TTL: Duration = Duration::from_secs(1);
const MAX_POPUP_CONTENT_WIDTH: usize = 48;

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
    match level {
        NotificationLevel::Info => tracing::info!("{}", message),
        NotificationLevel::Warn => tracing::warn!("{}", message),
        NotificationLevel::Error => tracing::error!("{}", message),
    }

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

    let popup_origin = Position::new(
        origin.row,
        origin.col + size.cols.saturating_sub(popup_width),
    );
    let popup_size = Size::new(popup_height, popup_width);

    let body_style = notification_body_style();
    if popup_size.rows > 2 && popup_size.cols > 2 {
        screen.fill_region(
            popup_origin.row + 1,
            popup_origin.col + 1,
            popup_size.rows - 2,
            popup_size.cols - 2,
            body_style,
        );
    }

    let border_style = level_style(message.level);
    render_popup_border(screen, popup_origin, popup_size, border_style);

    let text_col = popup_origin.col + 1;
    for (line_idx, line) in rendered_lines.iter().enumerate() {
        let row = popup_origin.row + 1 + line_idx as u16;
        if row >= popup_origin.row + popup_size.rows - 1 {
            break;
        }
        screen.write_string(row, text_col, border_style, line.as_str());
    }
}

fn render_popup_border(screen: &mut Screen, origin: Position, size: Size, style: Style) {
    let unicode_borders =
        globals::with_config(|config| config.unicode_borders_enabled()).unwrap_or(false);
    let (top_left, top_right, bottom_left, bottom_right, horizontal, vertical) =
        border_glyphs(unicode_borders);

    let top_row = origin.row;
    let bottom_row = origin.row + size.rows - 1;
    let left_col = origin.col;
    let right_col = origin.col + size.cols - 1;

    screen.write_string(top_row, left_col, style, top_left);
    screen.write_string(top_row, right_col, style, top_right);
    screen.write_string(bottom_row, left_col, style, bottom_left);
    screen.write_string(bottom_row, right_col, style, bottom_right);

    for col in left_col + 1..right_col {
        screen.write_string(top_row, col, style, horizontal);
        screen.write_string(bottom_row, col, style, horizontal);
    }

    for row in top_row + 1..bottom_row {
        screen.write_string(row, left_col, style, vertical);
        screen.write_string(row, right_col, style, vertical);
    }
}

fn border_glyphs(
    unicode_borders: bool,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    if unicode_borders {
        ("┌", "┐", "└", "┘", "─", "│")
    } else {
        ("+", "+", "+", "+", "-", "|")
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
