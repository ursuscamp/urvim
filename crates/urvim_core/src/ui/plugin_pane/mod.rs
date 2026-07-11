//! Retained plugin UI rendered as a split-tree pane.

use crate::screen::Screen;
use crate::ui::plugin_window::{PluginWindow, PluginWindowContent, PluginWindowOptions};
use crate::ui::text_width::{ClipSide, clip_text};
use crate::ui::{Intent, UiRect};
use crate::{editor, globals};
use urvim_terminal::{Color, Key, Style};
use urvim_theme::Tag;

/// Presentation options for a plugin pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPaneOptions {
    /// Optional centered header title.
    pub title: Option<String>,
    /// Theme tag for the pane body.
    pub body_style: Tag,
    /// Theme tag for the unfocused pane header.
    pub header_style: Tag,
    /// Theme tag for the focused pane header.
    pub focused_header_style: Tag,
}

impl Default for PluginPaneOptions {
    fn default() -> Self {
        Self {
            title: None,
            body_style: Tag::parse("ui.window").expect("built-in window tag should parse"),
            header_style: Tag::parse("ui.tab.inactive")
                .expect("built-in inactive tab tag should parse"),
            focused_header_style: Tag::parse("ui.tab.active")
                .expect("built-in active tab tag should parse"),
        }
    }
}

/// Plugin-owned retained UI hosted in a layout pane.
#[derive(Debug)]
pub struct PluginPane {
    options: PluginPaneOptions,
    window: PluginWindow,
}

impl PluginPane {
    /// Creates a retained plugin pane.
    pub fn new(owner: String, options: PluginPaneOptions) -> Self {
        let window_options = PluginWindowOptions {
            title: options.title.clone(),
            body_style: options.body_style.clone(),
            ..PluginWindowOptions::default()
        };
        Self {
            options,
            window: PluginWindow::new(owner, window_options),
        }
    }

    /// Returns the plugin that owns this pane.
    pub fn owner(&self) -> &str {
        self.window.owner()
    }

    /// Returns the pane presentation options.
    pub fn options(&self) -> &PluginPaneOptions {
        &self.options
    }

    /// Updates the pane presentation options.
    pub fn set_options(&mut self, options: PluginPaneOptions) {
        let mut window_options = self.window.options().clone();
        window_options.title = options.title.clone();
        window_options.body_style = options.body_style.clone();
        self.window.set_options(window_options);
        self.options = options;
    }

    /// Returns the retained pane content.
    pub fn content(&self) -> &PluginWindowContent {
        self.window.content()
    }

    /// Replaces the retained pane content.
    pub fn set_content(&mut self, content: PluginWindowContent) {
        self.window.set_content(content);
    }

    /// Installs a local keymap binding.
    pub fn set_keymap(&mut self, keys: Vec<String>, rhs: String, intent: Intent) {
        self.window.set_keymap(keys, rhs, intent);
    }

    /// Removes a local keymap binding.
    pub fn delete_keymap(&mut self, keys: &[String]) {
        self.window.delete_keymap(keys);
    }

    /// Returns all local keymap bindings.
    pub fn keymaps(&self) -> Vec<(Vec<String>, String)> {
        self.window.keymaps()
    }

    /// Routes a key through the local keymap.
    pub fn handle_key(&mut self, key: &Key) -> editor::HandleKeyResult {
        self.window.handle_key(key)
    }

    /// Clears any partially entered local key sequence.
    pub fn clear_pending_keys(&mut self) {
        self.window.clear_pending_keys();
    }

    /// Renders the pane header and retained content.
    pub fn render(&self, screen: &mut Screen, rect: UiRect, focused: bool) {
        if rect.size.rows == 0 || rect.size.cols == 0 {
            return;
        }

        let header_style = header_style(&self.options, focused);
        screen.fill_region(
            rect.origin.row,
            rect.origin.col,
            1,
            rect.size.cols,
            header_style,
        );
        if let Some(title) = self.options.title.as_deref() {
            let clipped = clip_text(title, rect.size.cols as usize, ClipSide::Center);
            let offset = (rect.size.cols as usize - clipped.width) / 2;
            screen.write_string(
                rect.origin.row,
                rect.origin.col.saturating_add(offset as u16),
                header_style,
                clipped.text.as_str(),
            );
        }

        let content_rows = rect.size.rows.saturating_sub(1);
        self.window.render_content_in_rect(
            screen,
            UiRect::new(
                crate::window::Position::new(rect.origin.row.saturating_add(1), rect.origin.col),
                crate::window::Size::new(content_rows, rect.size.cols),
            ),
        );
    }
}

fn header_style(options: &PluginPaneOptions, focused: bool) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| {
                let tag = if focused {
                    &options.focused_header_style
                } else {
                    &options.header_style
                };
                theme.resolve_name_with_default(tag.as_str())
            })
            .unwrap_or_else(|| {
                let inactive = Style::new().bg(Color::ansi(237)).fg(Color::ansi(250));
                if focused {
                    inactive.reverse().bold()
                } else {
                    inactive
                }
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::plugin_window::PluginWindowSegment;
    use crate::window::{Position, Size};
    use urvim_terminal::Color;
    use urvim_theme::{HighlightStyles, Theme, ThemeKind};

    fn pane_theme() -> Theme {
        let default_style = Style::new().fg(Color::ansi(1)).bg(Color::ansi(2));
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.tab.active").unwrap(),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        );
        highlights.insert(
            Tag::parse("ui.tab.inactive").unwrap(),
            Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        );
        highlights.insert(
            Tag::parse("ui.window").unwrap(),
            Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
        );
        highlights.insert(
            Tag::parse("ui.picker.location").unwrap(),
            Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        );
        highlights.insert(
            Tag::parse("ui.picker.accent").unwrap(),
            Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
        );
        Theme::new("pane", ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn plugin_pane_renders_centered_header_and_full_width_content() {
        let theme = pane_theme();
        let inactive_style = theme.resolve_name_with_default("ui.picker.location");
        let body_style = theme.resolve_name_with_default("ui.window");
        let _theme_guard = globals::set_test_active_theme(theme);
        let mut pane = PluginPane::new(
            "demo".to_string(),
            PluginPaneOptions {
                title: Some("猫ab".to_string()),
                header_style: Tag::parse("ui.picker.location").unwrap(),
                ..PluginPaneOptions::default()
            },
        );
        pane.set_content(vec![vec![PluginWindowSegment {
            text: "content!".to_string(),
            style: None,
        }]]);
        let mut screen = Screen::new(3, 8);

        pane.render(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(3, 8)),
            false,
        );

        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "猫");
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, inactive_style);
        assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, "c");
        assert_eq!(screen.get_cell_mut(1, 7).unwrap().text, "!");
        assert_eq!(screen.get_cell_mut(1, 7).unwrap().style, body_style);
        assert_eq!(screen.get_cell_mut(2, 0).unwrap().style, body_style);
    }

    #[test]
    fn plugin_pane_header_uses_active_style_and_clips_wide_title() {
        let theme = pane_theme();
        let active_style = theme.resolve_name_with_default("ui.picker.accent");
        let _theme_guard = globals::set_test_active_theme(theme);
        let pane = PluginPane::new(
            "demo".to_string(),
            PluginPaneOptions {
                title: Some("a猫bc".to_string()),
                focused_header_style: Tag::parse("ui.picker.accent").unwrap(),
                ..PluginPaneOptions::default()
            },
        );
        let mut screen = Screen::new(1, 3);

        pane.render(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(1, 3)),
            true,
        );

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "a");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "c");
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, active_style);
    }

    #[test]
    fn plugin_pane_handles_empty_rectangles() {
        let pane = PluginPane::new("demo".to_string(), PluginPaneOptions::default());
        let mut screen = Screen::new(1, 1);

        pane.render(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(0, 1)),
            true,
        );
        pane.render(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(1, 0)),
            true,
        );
    }
}
