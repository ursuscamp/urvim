//! Notification banner rendering.

use crate::globals;
use crate::screen::Screen;
use urvim_terminal::{Color, Style};
use crate::ui::floating_window::{FloatingAnchor, FloatingWindowFrame};
use crate::window::{Position, Size};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{NotificationLevel, NotificationMessage};

const MAX_POPUP_CONTENT_WIDTH: usize = 48;

/// Renders the currently active notification as a top-right floating popup.
pub fn render_active_banner(
    screen: &mut Screen,
    origin: Position,
    size: Size,
    now: std::time::Instant,
) {
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
            std::time::Instant::now(),
            std::time::Duration::from_secs(3),
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
            std::time::Instant::now(),
            std::time::Duration::from_secs(3),
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
