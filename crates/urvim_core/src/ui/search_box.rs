//! Traditional two-field search and replacement widgets.

use crate::buffer::{SearchDirection, SearchOptions, SearchPattern};
use crate::screen::Screen;
use crate::ui::inputs::{InputWidget, PromptSegment};
use crate::ui::line_format::{FormattedLineSection, FormattedLineSegment, FormattedLineTemplate};
use crate::ui::overlay::frame::{
    OverlayAnchor, OverlayFrame, OverlayFrameLabel, OverlayMargins, OverlayPlacement,
};
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use urvim_terminal::{KeyCode, Style};

/// Result produced by the search box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchBoxOutcome {
    /// Commit a search-only request.
    Search {
        /// Submitted search query.
        query: String,
        /// Retained replacement text while Replace mode is disabled.
        replacement: String,
        /// Options selected for the query.
        options: SearchOptions,
    },
    /// Begin replacing the submitted search matches.
    Replace {
        /// Submitted search query.
        query: String,
        /// Replacement text.
        replacement: String,
        /// Options selected for the query.
        options: SearchOptions,
    },
    /// Restore the search that was active before the box opened.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchField {
    Search,
    Replace,
}

/// A centered search form with an optional replacement input.
#[derive(Debug)]
pub struct SearchBox {
    search: InputWidget,
    replacement: InputWidget,
    focused: SearchField,
    options: SearchOptions,
    replace_enabled: bool,
    outcome: Option<SearchBoxOutcome>,
}

impl SearchBox {
    /// Creates a search form prefilled with the previous values.
    pub fn new(
        query: impl Into<String>,
        replacement: impl Into<String>,
        options: SearchOptions,
        replace_enabled: bool,
    ) -> Self {
        let prompt_style = highlight_style("ui.input.prompt");
        let mut search = InputWidget::new(query);
        search.set_prompt_segments(vec![
            PromptSegment::new("Search:", prompt_style),
            PromptSegment::new("  ", Style::default()),
        ]);
        let mut replacement = InputWidget::new(replacement);
        replacement.set_prompt_segments(vec![
            PromptSegment::new("Replace:", prompt_style),
            PromptSegment::new(" ", Style::default()),
        ]);
        Self {
            search,
            replacement,
            focused: SearchField::Search,
            options,
            replace_enabled,
            outcome: None,
        }
    }

    /// Returns the live search text.
    pub fn query(&self) -> &str {
        self.search.text()
    }

    /// Returns the replacement text.
    pub fn replacement(&self) -> &str {
        self.replacement.text()
    }

    /// Returns the currently selected search options.
    pub fn options(&self) -> SearchOptions {
        self.options
    }

    /// Returns whether replacement mode is enabled.
    pub fn replace_enabled(&self) -> bool {
        self.replace_enabled
    }

    /// Returns the currently rendered input cursor.
    pub fn cursor(&self) -> Option<crate::ui::geometry::Position> {
        match self.focused {
            SearchField::Search => self.search.render_cursor(),
            SearchField::Replace => self.replacement.render_cursor(),
        }
    }

    /// Takes a completed outcome.
    pub fn take_outcome(&mut self) -> Option<SearchBoxOutcome> {
        self.outcome.take()
    }

    /// Handles form input.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        match event {
            UiEvent::Key(key) if key.modifiers.has_ctrl() => match key.code {
                KeyCode::Char('r') => {
                    self.options
                        .set_direction(self.options.direction().opposite());
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Char('c') => {
                    self.options
                        .set_case_sensitive(!self.options.case_sensitive());
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Char('e') => {
                    self.options.set_regex(!self.options.regex());
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Char('p') => {
                    self.replace_enabled = !self.replace_enabled;
                    if !self.replace_enabled {
                        self.focused = SearchField::Search;
                    }
                    UiEventResult::Handled(Vec::new())
                }
                _ => UiEventResult::NotHandled,
            },
            UiEvent::Key(key) => match key.code {
                KeyCode::Esc => {
                    self.outcome = Some(SearchBoxOutcome::Cancelled);
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Enter => {
                    if !self.query_is_valid() {
                        return UiEventResult::Handled(Vec::new());
                    }
                    self.outcome = Some(if self.replace_enabled {
                        SearchBoxOutcome::Replace {
                            query: self.query().to_string(),
                            replacement: self.replacement().to_string(),
                            options: self.options,
                        }
                    } else {
                        SearchBoxOutcome::Search {
                            query: self.query().to_string(),
                            replacement: self.replacement().to_string(),
                            options: self.options,
                        }
                    });
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Tab => {
                    if self.replace_enabled {
                        self.focused = match self.focused {
                            SearchField::Search => SearchField::Replace,
                            SearchField::Replace => SearchField::Search,
                        };
                    } else {
                        self.focused = SearchField::Search;
                    }
                    UiEventResult::Handled(Vec::new())
                }
                _ => {
                    let handled = match self.focused {
                        SearchField::Search => self.search.handle_key(*key),
                        SearchField::Replace => self.replacement.handle_key(*key),
                    };
                    if handled {
                        UiEventResult::Handled(Vec::new())
                    } else {
                        UiEventResult::NotHandled
                    }
                }
            },
            UiEvent::Paste(text) => {
                let text = normalize_single_line(text);
                match self.focused {
                    SearchField::Search => self.search.insert_str(&text),
                    SearchField::Replace => self.replacement.insert_str(&text),
                };
                UiEventResult::Handled(Vec::new())
            }
            UiEvent::Resize(_, _) | UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    fn query_is_valid(&self) -> bool {
        SearchPattern::compile(self.query(), self.options).is_ok()
    }

    fn refresh_search_prompt(&mut self) {
        let style = if self.query_is_valid() {
            highlight_style("ui.input.prompt")
        } else {
            highlight_style("ui.diagnostic.error")
        };
        self.search.set_prompt_segments(vec![
            PromptSegment::new("Search:", style),
            PromptSegment::new("  ", Style::default()),
        ]);
    }

    /// Renders the form.
    pub fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        let content_rows = if self.replace_enabled { 5 } else { 4 };
        if rect.size.rows < content_rows + 2 || rect.size.cols < 3 {
            return;
        }
        let body = theme_style("ui.window");
        let border = theme_style("ui.window.lines.border");
        self.refresh_search_prompt();
        self.search.set_text_style(body);
        self.replacement.set_text_style(body);
        let Some(frame) = OverlayFrame::resolve_placement(
            rect.origin,
            rect.size,
            content_rows,
            rect.size.cols.min(55).saturating_sub(2),
            OverlayPlacement::Anchored {
                anchor: OverlayAnchor::Center,
                margins: OverlayMargins::default(),
            },
        ) else {
            return;
        };
        let title = if self.replace_enabled {
            "Search and Replace"
        } else {
            "Search"
        };
        frame.render_bordered_with_label(
            screen,
            border,
            body,
            Some(OverlayFrameLabel::top_center(title)),
        );
        self.search.render_widget(
            screen,
            UiRect::new(
                frame.content_origin,
                crate::ui::Size::new(1, frame.content_size.cols),
            ),
            &UiContext,
        );
        if self.replace_enabled {
            self.replacement.render_widget(
                screen,
                UiRect::new(
                    crate::ui::Position::new(
                        frame.content_origin.row + 1,
                        frame.content_origin.col,
                    ),
                    crate::ui::Size::new(1, frame.content_size.cols),
                ),
                &UiContext,
            );
        }
        let toggle_row = frame.content_origin.row + if self.replace_enabled { 3 } else { 2 };
        let key_style = body.accent(theme_style("ui.input.prompt"));
        let enabled_style = body.accent(theme_style("ui.picker.accent"));
        let disabled_style = body.accent(theme_style("ui.picker.location")).faint();
        let segments = toggle_line_segments(
            self.options,
            frame.content_size.cols,
            body,
            key_style,
            enabled_style,
            disabled_style,
        );
        render_line_segments(screen, toggle_row, frame.content_origin.col, segments);
        let segments = mode_toggle_line_segments(
            self.options,
            self.replace_enabled,
            frame.content_size.cols,
            body,
            key_style,
            enabled_style,
            disabled_style,
        );
        render_line_segments(screen, toggle_row + 1, frame.content_origin.col, segments);
    }
}

impl Widget for SearchBox {
    fn handle_ui_event(&mut self, event: &UiEvent, ctx: &mut UiContext) -> UiEventResult {
        SearchBox::handle_ui_event(self, event, ctx)
    }
    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext) {
        SearchBox::render_widget(self, screen, rect, ctx)
    }
    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

/// Choice made while confirming replacements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceChoice {
    /// Replace the current match and continue.
    Replace,
    /// Leave the current match unchanged and continue.
    Skip,
    /// Replace the current match and every remaining match.
    All,
    /// Stop while preserving replacements already made.
    Cancel,
}

/// Compact confirmation prompt shown for each replacement.
#[derive(Debug, Default)]
pub struct ReplaceConfirmation {
    choice: Option<ReplaceChoice>,
}

impl ReplaceConfirmation {
    /// Takes the latest choice.
    pub fn take_choice(&mut self) -> Option<ReplaceChoice> {
        self.choice.take()
    }
}

impl Widget for ReplaceConfirmation {
    fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        let UiEvent::Key(key) = event else {
            return if matches!(event, UiEvent::Paste(_)) {
                UiEventResult::Handled(Vec::new())
            } else {
                UiEventResult::NotHandled
            };
        };
        self.choice = match key.code {
            KeyCode::Enter | KeyCode::Char('r') => Some(ReplaceChoice::Replace),
            KeyCode::Char('s') | KeyCode::Char('n') => Some(ReplaceChoice::Skip),
            KeyCode::Char('a') => Some(ReplaceChoice::All),
            KeyCode::Esc => Some(ReplaceChoice::Cancel),
            _ => None,
        };
        if self.choice.is_some() {
            UiEventResult::Handled(Vec::new())
        } else {
            UiEventResult::NotHandled
        }
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        let body = theme_style("ui.window");
        let border = theme_style("ui.window.lines.border");
        let Some(frame) = OverlayFrame::resolve_placement(
            rect.origin,
            rect.size,
            1,
            rect.size.cols.min(58).saturating_sub(2),
            OverlayPlacement::Anchored {
                anchor: OverlayAnchor::Center,
                margins: OverlayMargins::default(),
            },
        ) else {
            return;
        };
        frame.render_bordered_with_label(
            screen,
            border,
            body,
            Some(OverlayFrameLabel::top_center("Replace match?")),
        );
        screen.write_string(
            frame.content_origin.row,
            frame.content_origin.col,
            body,
            "[Enter/r] Replace  [s/n] Skip  [a] All  [Esc] Stop",
        );
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

fn toggle_line_segments(
    options: SearchOptions,
    available_width: u16,
    body_style: Style,
    key_style: Style,
    enabled_style: Style,
    disabled_style: Style,
) -> Vec<FormattedLineSegment> {
    formatted_toggle_line(
        ToggleLineItem {
            key: "[Ctrl-R]",
            label: " Reverse: ",
            enabled: options.direction() == SearchDirection::Reverse,
        },
        ToggleLineItem {
            key: "[Ctrl-C]",
            label: " Case Sensitive: ",
            enabled: options.case_sensitive(),
        },
        available_width,
        body_style,
        key_style,
        enabled_style,
        disabled_style,
    )
}

fn mode_toggle_line_segments(
    options: SearchOptions,
    replace_enabled: bool,
    available_width: u16,
    body_style: Style,
    key_style: Style,
    enabled_style: Style,
    disabled_style: Style,
) -> Vec<FormattedLineSegment> {
    formatted_toggle_line(
        ToggleLineItem {
            key: "[Ctrl-E]",
            label: " Regex: ",
            enabled: options.regex(),
        },
        ToggleLineItem {
            key: "[Ctrl-P]",
            label: " Replace: ",
            enabled: replace_enabled,
        },
        available_width,
        body_style,
        key_style,
        enabled_style,
        disabled_style,
    )
}

#[derive(Debug, Clone, Copy)]
struct ToggleLineItem<'a> {
    key: &'a str,
    label: &'a str,
    enabled: bool,
}

fn formatted_toggle_line(
    left: ToggleLineItem<'_>,
    right: ToggleLineItem<'_>,
    available_width: u16,
    body_style: Style,
    key_style: Style,
    enabled_style: Style,
    disabled_style: Style,
) -> Vec<FormattedLineSegment> {
    FormattedLineTemplate::new(vec![
        FormattedLineSection::fixed(8, key_style),
        FormattedLineSection::fixed(10, body_style),
        FormattedLineSection::fixed(
            3,
            if left.enabled {
                enabled_style
            } else {
                disabled_style
            },
        ),
        FormattedLineSection::fixed(2, body_style),
        FormattedLineSection::fixed(8, key_style),
        FormattedLineSection::fixed(17, body_style),
        FormattedLineSection::fixed(
            3,
            if right.enabled {
                enabled_style
            } else {
                disabled_style
            },
        ),
    ])
    .render_segments(
        [
            left.key,
            left.label,
            if left.enabled { "ON" } else { "OFF" },
            "",
            right.key,
            right.label,
            if right.enabled { "ON" } else { "OFF" },
        ],
        available_width,
    )
    .unwrap_or_default()
}

fn render_line_segments(
    screen: &mut Screen,
    row: u16,
    col: u16,
    segments: Vec<FormattedLineSegment>,
) {
    let mut current_col = col;
    for segment in segments {
        screen.write_string(row, current_col, segment.style, segment.text.as_str());
        current_col = current_col
            .saturating_add(crate::ui::text_width::display_width(segment.text.as_str()) as u16);
    }
}

fn normalize_single_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

fn highlight_style(name: &str) -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(name))
            .unwrap_or_default()
    })
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
    use super::*;
    use urvim_terminal::{Color, Key, KeyCode, Modifiers};
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    #[test]
    fn search_inputs_use_the_themed_prompt_style() {
        let prompt_style = Style::new().fg(Color::ansi(2)).bold();
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.input.prompt").expect("valid tag"),
            prompt_style,
        );
        let _theme_guard = crate::globals::set_test_active_theme(Theme::new(
            "search-test",
            ThemeKind::Ansi256,
            Style::default(),
            highlights,
        ));

        let widget = SearchBox::new("one", "two", SearchOptions::default(), false);

        assert_eq!(widget.search.prompt(), "Search:  ");
        assert_eq!(widget.search.prompt_segments()[0].style, prompt_style);
        assert_eq!(widget.replacement.prompt(), "Replace: ");
        assert_eq!(widget.replacement.prompt_segments()[0].style, prompt_style);
    }

    #[test]
    fn toggle_line_formats_keys_and_statuses_with_distinct_styles() {
        let body = Style::new().fg(Color::ansi(1));
        let key = Style::new().fg(Color::ansi(2));
        let enabled = Style::new().fg(Color::ansi(3));
        let disabled = Style::new().fg(Color::ansi(4));
        let segments =
            toggle_line_segments(SearchOptions::default(), 53, body, key, enabled, disabled);
        let text = segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<String>();

        assert_eq!(text, "[Ctrl-R] Reverse: OFF  [Ctrl-C] Case Sensitive: ON ");
        assert_eq!(segments[0].style, key);
        assert_eq!(segments[2].style, disabled);
        assert_eq!(segments[4].style, key);
        assert_eq!(segments[6].style, enabled);
    }

    #[test]
    fn control_shortcuts_toggle_search_options_without_changing_fields() {
        let mut widget = SearchBox::new("one", "two", SearchOptions::default(), false);
        widget.handle_ui_event(
            &UiEvent::Key(Key {
                code: KeyCode::Char('r'),
                modifiers: Modifiers::CTRL,
            }),
            &mut UiContext,
        );
        widget.handle_ui_event(
            &UiEvent::Key(Key {
                code: KeyCode::Char('c'),
                modifiers: Modifiers::CTRL,
            }),
            &mut UiContext,
        );
        widget.handle_ui_event(
            &UiEvent::Key(Key {
                code: KeyCode::Char('e'),
                modifiers: Modifiers::CTRL,
            }),
            &mut UiContext,
        );
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Enter.key()), &mut UiContext);

        assert_eq!(
            widget.take_outcome(),
            Some(SearchBoxOutcome::Search {
                query: "one".into(),
                replacement: "two".into(),
                options: SearchOptions::new(SearchDirection::Reverse, false, true),
            })
        );
        assert_eq!(widget.replacement(), "two");
    }

    #[test]
    fn invalid_regex_uses_error_prompt_and_blocks_enter() {
        let prompt_style = Style::new().fg(Color::ansi(2));
        let error_style = Style::new().fg(Color::ansi(1)).bold();
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.input.prompt").expect("valid tag"),
            prompt_style,
        );
        highlights.insert(
            Tag::parse("ui.diagnostic.error").expect("valid tag"),
            error_style,
        );
        let _theme_guard = crate::globals::set_test_active_theme(Theme::new(
            "search-error-test",
            ThemeKind::Ansi256,
            Style::default(),
            highlights,
        ));
        let options = SearchOptions::new(SearchDirection::Forward, true, true);
        let mut widget = SearchBox::new("(", "", options, false);

        widget.refresh_search_prompt();
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Enter.key()), &mut UiContext);

        assert_eq!(widget.search.prompt_segments()[0].style, error_style);
        assert_eq!(widget.take_outcome(), None);
    }

    #[test]
    fn toggle_rows_share_aligned_columns() {
        let style = Style::default();
        let options = SearchOptions::default();
        let primary = toggle_line_segments(options, 53, style, style, style, style)
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<String>();
        let modes = mode_toggle_line_segments(options, false, 53, style, style, style, style)
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<String>();

        assert_eq!(modes, "[Ctrl-E] Regex:   OFF  [Ctrl-P] Replace:        OFF");
        assert_eq!(primary.find("OFF"), modes.find("OFF"));
        assert_eq!(primary.find("[Ctrl-C]"), modes.find("[Ctrl-P]"));
        assert_eq!(primary.rfind("ON"), modes.rfind("OFF"));
    }

    #[test]
    fn replacement_input_is_rendered_only_when_replace_mode_is_enabled() {
        fn render_text(widget: &mut SearchBox) -> String {
            let mut screen = Screen::new(10, 80);
            widget.render_widget(
                &mut screen,
                UiRect::new(crate::ui::Position::new(0, 0), crate::ui::Size::new(10, 80)),
                &UiContext,
            );
            let mut text = String::new();
            for row in 0..10 {
                for col in 0..80 {
                    text.push_str(&screen.get_cell_mut(row, col).unwrap().text);
                }
                text.push('\n');
            }
            text
        }

        let mut disabled = SearchBox::new("one", "two", SearchOptions::default(), false);
        let mut enabled = SearchBox::new("one", "two", SearchOptions::default(), true);

        assert_eq!(render_text(&mut disabled).matches("Replace:").count(), 1);
        assert_eq!(render_text(&mut enabled).matches("Replace:").count(), 2);
    }

    #[test]
    fn replace_toggle_controls_visibility_focus_and_enter_behavior() {
        let mut widget = SearchBox::new("one", "two", SearchOptions::default(), true);
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Tab.key()), &mut UiContext);
        assert_eq!(widget.focused, SearchField::Replace);

        widget.handle_ui_event(
            &UiEvent::Key(Key {
                code: KeyCode::Char('p'),
                modifiers: Modifiers::CTRL,
            }),
            &mut UiContext,
        );
        assert!(!widget.replace_enabled());
        assert_eq!(widget.focused, SearchField::Search);
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Tab.key()), &mut UiContext);
        assert_eq!(widget.focused, SearchField::Search);
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Enter.key()), &mut UiContext);
        assert!(matches!(
            widget.take_outcome(),
            Some(SearchBoxOutcome::Search { .. })
        ));
    }

    #[test]
    fn enter_on_search_starts_replacement_when_enabled() {
        let mut widget = SearchBox::new("one", "two", SearchOptions::default(), true);

        widget.handle_ui_event(&UiEvent::Key(KeyCode::Enter.key()), &mut UiContext);

        assert_eq!(
            widget.take_outcome(),
            Some(SearchBoxOutcome::Replace {
                query: "one".into(),
                replacement: "two".into(),
                options: SearchOptions::default(),
            })
        );
    }

    #[test]
    fn tab_and_enter_start_replacement() {
        let mut widget = SearchBox::new("one", "", SearchOptions::default(), true);
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Tab.key()), &mut UiContext);
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Char('x').key()), &mut UiContext);
        widget.handle_ui_event(&UiEvent::Key(KeyCode::Enter.key()), &mut UiContext);
        assert_eq!(
            widget.take_outcome(),
            Some(SearchBoxOutcome::Replace {
                query: "one".into(),
                replacement: "x".into(),
                options: SearchOptions::default(),
            })
        );
    }
}
