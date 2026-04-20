//! Resolved theme models and the in-memory theme registry.

use crate::terminal::{Color, Style};
use std::collections::BTreeMap;

use super::error::ThemeLoadError;
use super::loader::resolve_theme_from_str;
use super::tag::Tag;

/// Indicates whether a theme is ANSI 256-color or true color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// All palette entries use ANSI 256-color values.
    Ansi256,
    /// At least one palette entry uses a true RGB value.
    TrueColor,
}

/// A partially specified style that can be layered onto another style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleOverlay {
    /// Optional foreground color overlay.
    pub fg: Option<Color>,
    /// Optional background color overlay.
    pub bg: Option<Color>,
    /// Optional underline color overlay.
    pub underline_color: Option<Color>,
    /// Optional bold overlay.
    pub bold: Option<bool>,
    /// Optional italic overlay.
    pub italic: Option<bool>,
    /// Optional underline overlay.
    pub underline: Option<bool>,
    /// Optional double underline overlay.
    pub double_underline: Option<bool>,
    /// Optional dim overlay.
    pub dim: Option<bool>,
    /// Optional reverse overlay.
    pub reverse: Option<bool>,
    /// Optional blink overlay.
    pub blink: Option<bool>,
    /// Optional strikethrough overlay.
    pub strikethrough: Option<bool>,
    /// Optional overline overlay.
    pub overline: Option<bool>,
}

impl StyleOverlay {
    /// Applies this overlay to an existing style.
    pub fn apply_to(self, style: Style) -> Style {
        let mut style = style;

        if let Some(color) = self.fg {
            style = style.set_foreground(Some(color));
        }
        if let Some(color) = self.bg {
            style = style.set_background(Some(color));
        }
        if let Some(color) = self.underline_color {
            style = style.set_underline_color(Some(color));
        }
        if let Some(enabled) = self.bold {
            style = style.set_bold(enabled);
        }
        if let Some(enabled) = self.italic {
            style = style.set_italic(enabled);
        }
        if let Some(enabled) = self.underline {
            style = style.set_underline(enabled);
        }
        if let Some(enabled) = self.double_underline {
            style = style.set_double_underline(enabled);
        }
        if let Some(enabled) = self.dim {
            style = style.set_faint(enabled);
        }
        if let Some(enabled) = self.reverse {
            style = style.set_reverse(enabled);
        }
        if let Some(enabled) = self.blink {
            style = style.set_blink(enabled);
        }
        if let Some(enabled) = self.strikethrough {
            style = style.set_strikethrough(enabled);
        }
        if let Some(enabled) = self.overline {
            style = style.set_overline(enabled);
        }

        style
    }
}

/// Fully resolved styles keyed by hierarchical highlight names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HighlightStyles {
    styles: BTreeMap<Tag, Style>,
}

impl HighlightStyles {
    /// Creates a new highlight style map.
    pub fn new(styles: BTreeMap<Tag, Style>) -> Self {
        Self { styles }
    }

    /// Returns the resolved style for a tag after specificity fallback.
    pub fn style_for_tag(&self, tag: &Tag, default_style: Style) -> Style {
        if let Some(style) = self.try_style_for_tag(tag) {
            return style;
        }

        if let Some(prefixed_tag) = prefixed_syntax_tag(tag)
            && let Some(style) = self.try_style_for_tag(&prefixed_tag)
        {
            return style;
        }

        default_style
    }

    /// Returns the resolved style for a dot-separated highlight name.
    pub fn style_for_name(&self, name: &str, default_style: Style) -> Style {
        Tag::parse(name)
            .map(|tag| self.style_for_tag(&tag, default_style))
            .unwrap_or(default_style)
    }

    /// Inserts a resolved style for a tag.
    pub fn insert(&mut self, tag: Tag, style: Style) {
        self.styles.insert(tag, style);
    }

    /// Returns an iterator over the stored tag styles.
    pub fn iter(&self) -> impl Iterator<Item = (&Tag, &Style)> {
        self.styles.iter()
    }

    fn try_style_for_tag(&self, tag: &Tag) -> Option<Style> {
        for candidate in tag.parent_chain() {
            if let Some(style) = self
                .styles
                .get(&Tag::parse(candidate).expect("parent chain must yield valid tags"))
            {
                return Some(*style);
            }
        }

        None
    }
}

fn prefixed_syntax_tag(tag: &Tag) -> Option<Tag> {
    let value = tag.as_str();
    if value.starts_with("ui.") || value.starts_with("syntax.") {
        return None;
    }

    Tag::parse(&format!("syntax.{value}")).ok()
}

/// A fully resolved theme ready for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Resolved highlight styles used by rendering and syntax highlighting code.
    pub highlights: HighlightStyles,
    name: String,
    kind: ThemeKind,
    default_style: Style,
}

impl Theme {
    /// Creates a new resolved theme.
    pub fn new(
        name: impl Into<String>,
        kind: ThemeKind,
        default_style: Style,
        highlights: HighlightStyles,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            default_style,
            highlights,
        }
    }

    /// Returns the theme name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the theme color kind.
    pub fn kind(&self) -> ThemeKind {
        self.kind
    }

    /// Returns the theme default style.
    pub fn default_style(&self) -> Style {
        self.default_style
    }

    /// Returns the resolved style for a tag.
    pub fn highlight_style_for_tag(&self, tag: &Tag) -> Style {
        self.highlights.style_for_tag(tag, self.default_style)
    }

    /// Returns the resolved style for a dot-separated highlight name.
    pub fn highlight_style_for_name(&self, name: &str) -> Style {
        self.highlights.style_for_name(name, self.default_style)
    }
}

/// In-memory registry of resolved themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRegistry {
    themes: BTreeMap<String, Theme>,
    default_theme_name: String,
}

impl ThemeRegistry {
    /// Loads all built-in themes from embedded TOML sources.
    pub fn load_builtin() -> Result<Self, ThemeLoadError> {
        let mut sources = builtin_theme_sources().into_iter();
        let (default_name, source_name, source) = sources
            .next()
            .expect("builtin theme sources must contain a default theme");

        let default_theme = resolve_theme_from_str(source_name, source)?;
        if default_theme.name() != default_name {
            return Err(ThemeLoadError::InvalidThemeName(
                default_theme.name().to_string(),
            ));
        }

        let mut registry = Self::new(default_theme);
        for (expected_name, source_name, source) in sources {
            let theme = resolve_theme_from_str(source_name, source)?;
            if theme.name() != expected_name {
                return Err(ThemeLoadError::InvalidThemeName(theme.name().to_string()));
            }
            registry.insert(theme)?;
        }

        Ok(registry)
    }

    /// Creates a registry with the provided default theme already loaded.
    pub fn new(default_theme: Theme) -> Self {
        let default_theme_name = default_theme.name().to_string();
        let mut themes = BTreeMap::new();
        themes.insert(default_theme_name.clone(), default_theme);

        Self {
            themes,
            default_theme_name,
        }
    }

    /// Inserts a theme into the registry.
    pub fn insert(&mut self, theme: Theme) -> Result<(), ThemeLoadError> {
        let name = theme.name().to_string();
        if self.themes.contains_key(&name) {
            return Err(ThemeLoadError::DuplicateThemeName(name));
        }

        self.themes.insert(name, theme);
        Ok(())
    }

    /// Looks up a theme by name.
    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// Returns the default theme.
    pub fn default_theme(&self) -> &Theme {
        self.themes
            .get(&self.default_theme_name)
            .expect("theme registry must contain its default theme")
    }

    /// Returns the registered theme names in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.themes.keys().map(String::as_str).collect()
    }
}

fn builtin_theme_sources() -> [(&'static str, &'static str, &'static str); 6] {
    [
        (
            "Friday Night",
            "friday-night.toml",
            include_str!("builtin/friday-night.toml"),
        ),
        (
            "Saturday Morning",
            "saturday-morning.toml",
            include_str!("builtin/saturday-morning.toml"),
        ),
        (
            "Rose Pine",
            "rose-pine.toml",
            include_str!("builtin/rose-pine.toml"),
        ),
        (
            "Dracula",
            "dracula.toml",
            include_str!("builtin/dracula.toml"),
        ),
        (
            "Tokyo Night",
            "tokyo-night.toml",
            include_str!("builtin/tokyo-night.toml"),
        ),
        (
            "Catppuccin",
            "catppuccin.toml",
            include_str!("builtin/catppuccin.toml"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(value: &str) -> Tag {
        Tag::parse(value).expect("valid tag")
    }

    fn theme(name: &str) -> Theme {
        let default_style = Style::new().bold();
        let mut highlights = HighlightStyles::default();
        highlights.insert(tag("ui.status_bar"), Style::new().fg(Color::ansi(1)));
        highlights.insert(
            tag("ui.status_bar.modified_marker"),
            Style::new().fg(Color::ansi(2)),
        );
        highlights.insert(tag("ui.selection"), Style::new().reverse());
        highlights.insert(
            tag("ui.window.active_line"),
            Style::new().bg(Color::ansi(21)),
        );
        highlights.insert(tag("ui.tab.active"), Style::new().fg(Color::ansi(3)));
        highlights.insert(tag("ui.tab.inactive"), Style::new().fg(Color::ansi(4)));
        highlights.insert(
            tag("ui.tab.scroll_indicator"),
            Style::new().fg(Color::ansi(5)),
        );
        highlights.insert(tag("ui.window.gutter"), Style::new().fg(Color::ansi(6)));
        highlights.insert(tag("ui.window"), Style::new().fg(Color::ansi(7)));
        highlights.insert(
            tag("ui.window.split_border"),
            Style::new().fg(Color::ansi(8)),
        );
        highlights.insert(
            tag("ui.window.split_border.resize"),
            Style::new().fg(Color::ansi(9)),
        );
        highlights.insert(
            tag("syntax.comment"),
            Style::new().bold().fg(Color::ansi(10)),
        );
        highlights.insert(
            tag("syntax.constant"),
            Style::new().bold().fg(Color::ansi(11)),
        );
        highlights.insert(
            tag("syntax.function"),
            Style::new().bold().fg(Color::ansi(12)),
        );
        highlights.insert(
            tag("syntax.keyword"),
            Style::new().bold().fg(Color::ansi(13)),
        );
        highlights.insert(
            tag("syntax.markup.code"),
            Style::new().bold().fg(Color::ansi(20)),
        );
        highlights.insert(
            tag("syntax.operator"),
            Style::new().bold().fg(Color::ansi(15)),
        );
        highlights.insert(
            tag("syntax.punctuation"),
            Style::new().bold().fg(Color::ansi(16)),
        );
        highlights.insert(
            tag("syntax.string"),
            Style::new().bold().fg(Color::ansi(17)),
        );
        highlights.insert(
            tag("syntax.string.interpolation"),
            Style::new().bold().fg(Color::ansi(21)),
        );
        highlights.insert(tag("syntax.type"), Style::new().bold().fg(Color::ansi(18)));
        highlights.insert(
            tag("syntax.variable"),
            Style::new().bold().fg(Color::ansi(19)),
        );
        highlights.insert(
            tag("syntax.constant.integer"),
            Style::new().bold().fg(Color::ansi(14)),
        );

        Theme::new(name, ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn theme_returns_predefined_highlight_styles() {
        let theme = theme("demo");

        assert_eq!(
            theme.highlight_style_for_name("ui.status_bar"),
            Style::new().fg(Color::ansi(1))
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.status_bar.modified_marker"),
            Style::new().fg(Color::ansi(2))
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window.active_line"),
            Style::new().bg(Color::ansi(21))
        );
        assert_eq!(
            theme.highlight_style_for_name("ui.window"),
            Style::new().fg(Color::ansi(7))
        );
    }

    #[test]
    fn theme_returns_highlight_styles() {
        let theme = theme("demo");

        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.comment")),
            Style::new().bold().fg(Color::ansi(10))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.constant.integer")),
            Style::new().bold().fg(Color::ansi(14))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.constant.float")),
            Style::new().bold().fg(Color::ansi(11))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.markup.code.inline")),
            Style::new().bold().fg(Color::ansi(20))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.string.interpolation")),
            Style::new().bold().fg(Color::ansi(21))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.comment.todo")),
            Style::new().bold().fg(Color::ansi(10))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.comment.fixme")),
            Style::new().bold().fg(Color::ansi(10))
        );
    }

    #[test]
    fn theme_returns_marker_specific_comment_styles() {
        let mut theme = theme("demo");
        theme
            .highlights
            .insert(tag("syntax.comment.todo"), Style::new().fg(Color::ansi(22)));
        theme.highlights.insert(
            tag("syntax.comment.fixme"),
            Style::new().fg(Color::ansi(23)),
        );

        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.comment.todo")),
            Style::new().fg(Color::ansi(22))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.comment.fixme")),
            Style::new().fg(Color::ansi(23))
        );
        assert_eq!(
            theme.highlight_style_for_tag(&tag("syntax.comment.note")),
            Style::new().bold().fg(Color::ansi(10))
        );
    }

    #[test]
    fn registry_keeps_default_theme_and_detects_duplicates() {
        let default_theme = theme("default");
        let mut registry = ThemeRegistry::new(default_theme);

        assert_eq!(registry.default_theme().name(), "default");
        assert_eq!(registry.names(), vec!["default"]);

        let duplicate = theme("default");
        let err = registry
            .insert(duplicate)
            .expect_err("duplicate names should fail");
        assert_eq!(
            err,
            ThemeLoadError::DuplicateThemeName(String::from("default"))
        );
    }

    #[test]
    fn builtin_registry_loads_friday_night_as_default() {
        let registry = ThemeRegistry::load_builtin().expect("builtins should load");

        assert_eq!(registry.default_theme().name(), "Friday Night");
        assert!(registry.get("Saturday Morning").is_some());
        assert!(registry.get("Rose Pine").is_some());
        assert!(registry.get("Dracula").is_some());
        assert!(registry.get("Tokyo Night").is_some());
        assert!(registry.get("Catppuccin").is_some());
    }
}
