//! Passive bottom-anchored guide for pending editor key sequences.

use crate::editor::KeyGuideSnapshot;
use crate::screen::Screen;
use crate::ui::overlay::frame::{OverlayFrame, OverlayFrameLabel};
use crate::ui::{FocusPolicy, UiContext, UiRect};
use crate::widget::Widget;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use urvim_terminal::Style;

/// Widget that renders the available continuations for a pending key sequence.
#[derive(Debug, Clone)]
pub struct KeyGuideWidget {
    snapshot: KeyGuideSnapshot,
}

impl KeyGuideWidget {
    /// Creates a guide for a pending key sequence.
    pub fn new(snapshot: KeyGuideSnapshot) -> Self {
        Self { snapshot }
    }

    fn render(&self, screen: &mut Screen, rect: UiRect) {
        if self.snapshot.entries.is_empty() || rect.size.rows < 6 || rect.size.cols < 12 {
            return;
        }

        let available_content_rows = usize::from(rect.size.rows / 2).saturating_sub(2).max(1);
        let desired_width = self
            .snapshot
            .entries
            .iter()
            .map(|entry| {
                UnicodeWidthStr::width(entry.key.as_str())
                    + 3
                    + UnicodeWidthStr::width(entry.description.as_str())
            })
            .max()
            .unwrap_or(1)
            .saturating_add(2)
            .max(18);
        let content_width = usize::from(rect.size.cols.saturating_sub(2));
        let mut columns = (content_width / desired_width).max(1);
        columns = columns.min(self.snapshot.entries.len());
        let mut rows = self
            .snapshot
            .entries
            .len()
            .div_ceil(columns)
            .min(available_content_rows);
        if self.snapshot.entries.len() > rows.saturating_mul(columns)
            && rows == available_content_rows
            && rows > 1
        {
            rows -= 1;
        }
        let shown = rows
            .saturating_mul(columns)
            .min(self.snapshot.entries.len());
        let hidden = self.snapshot.entries.len().saturating_sub(shown);
        let show_overflow = hidden > 0 && rows < available_content_rows;
        let content_rows = rows + usize::from(show_overflow);
        let frame_rows = content_rows as u16 + 2;
        let frame = OverlayFrame::resolve_placement(
            rect.origin,
            rect.size,
            content_rows as u16,
            rect.size.cols.saturating_sub(2),
            crate::ui::overlay::frame::OverlayPlacement::Fixed {
                row: rect.size.rows.saturating_sub(frame_rows),
                col: 0,
            },
        );
        let Some(frame) = frame else {
            return;
        };

        let body = theme_style("ui.window");
        let border = theme_style("ui.window.lines.border");
        let accent = theme_style("ui.picker.accent");
        let muted = theme_style("ui.picker.location");
        let title = if self.snapshot.prefix.is_empty() {
            "Keys".to_string()
        } else {
            format!("Keys: {}", self.snapshot.prefix.concat())
        };
        frame.render_bordered_with_label(
            screen,
            border,
            body,
            Some(OverlayFrameLabel::top_center(&title)),
        );

        let column_width = usize::from(frame.content_size.cols) / columns;
        for (index, entry) in self.snapshot.entries.iter().take(shown).enumerate() {
            let column = index / rows;
            let row = index % rows;
            let col = usize::from(frame.content_origin.col) + column * column_width;
            let key_width = self
                .snapshot
                .entries
                .iter()
                .skip(column * rows)
                .take(rows)
                .map(|entry| UnicodeWidthStr::width(entry.key.as_str()))
                .max()
                .unwrap_or(1);
            let key = format!("{:<width$}", entry.key, width = key_width);
            screen.write_str(
                frame.content_origin.row + row as u16,
                col as u16,
                accent,
                &key,
            );
            let description_col = col + key_width + 3;
            let description_width = column_width.saturating_sub(key_width + 4);
            let description = truncate(&entry.description, description_width);
            screen.write_str(
                frame.content_origin.row + row as u16,
                description_col as u16,
                body,
                &description,
            );
        }
        if show_overflow {
            let text = format!("+{hidden} more");
            screen.write_str(
                frame.content_origin.row + rows as u16,
                frame.content_origin.col,
                muted,
                &text,
            );
        }
    }
}

impl Widget for KeyGuideWidget {
    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        self.render(screen, rect);
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Passive
    }
}

fn truncate(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let suffix = if width > 1 { "…" } else { "" };
    let target = width.saturating_sub(UnicodeWidthStr::width(suffix));
    let mut result = String::new();
    let mut used = 0;
    for character in text.chars() {
        let character_width = character.width().unwrap_or(0);
        if used + character_width > target {
            break;
        }
        result.push(character);
        used += character_width;
    }
    result.push_str(suffix);
    result
}

fn theme_style(name: &str) -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_respects_unicode_display_width() {
        assert_eq!(truncate("definition", 5), "defi…");
        assert_eq!(truncate("界面", 3), "界…");
        assert_eq!(truncate("text", 0), "");
    }
}
