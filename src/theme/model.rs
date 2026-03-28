//! Resolved theme models and the in-memory theme registry.

use crate::terminal::{Color, Style};
use std::collections::BTreeMap;

use super::error::ThemeLoadError;
use super::loader::resolve_theme_from_str;

/// Indicates whether a theme is ANSI 256-color or true color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// All palette entries use ANSI 256-color values.
    Ansi256,
    /// At least one palette entry uses a true RGB value.
    TrueColor,
}

/// A partially specified style that can be layered onto the theme default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleOverride {
    /// Optional foreground color override.
    pub fg: Option<Color>,
    /// Optional background color override.
    pub bg: Option<Color>,
    /// Optional underline color override.
    pub underline_color: Option<Color>,
    /// Optional bold override.
    pub bold: Option<bool>,
    /// Optional italic override.
    pub italic: Option<bool>,
    /// Optional underline override.
    pub underline: Option<bool>,
    /// Optional double underline override.
    pub double_underline: Option<bool>,
    /// Optional dim override.
    pub dim: Option<bool>,
    /// Optional reverse override.
    pub reverse: Option<bool>,
    /// Optional blink override.
    pub blink: Option<bool>,
    /// Optional strikethrough override.
    pub strikethrough: Option<bool>,
    /// Optional overline override.
    pub overline: Option<bool>,
}

impl StyleOverride {
    /// Applies this override to an existing style.
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

/// Fully resolved UI styles for the closed urvim schema.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UiStyles {
    /// Style used by the status bar.
    pub status_bar: Style,
    /// Style used by modified-buffer markers.
    pub modified_marker: Style,
    /// Style used by the active tab.
    pub tab_active: Style,
    /// Style used by inactive tabs.
    pub tab_inactive: Style,
    /// Style used by the tab bar scroll indicators.
    pub tab_scroll_indicator: Style,
    /// Style used by the window gutter.
    pub gutter: Style,
    /// Style used by the main window background.
    pub window: Style,
}

impl UiStyles {
    /// Creates a new set of fully resolved UI styles.
    pub fn new(
        status_bar: Style,
        modified_marker: Style,
        tab_active: Style,
        tab_inactive: Style,
        tab_scroll_indicator: Style,
        gutter: Style,
        window: Style,
    ) -> Self {
        Self {
            status_bar,
            modified_marker,
            tab_active,
            tab_inactive,
            tab_scroll_indicator,
            gutter,
            window,
        }
    }
}

/// Fully resolved syntax styles for the closed urvim schema.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyntaxStyles {
    /// Style used for comments.
    pub comment: Style,
    /// Style used for constants.
    pub constant: Style,
    /// Style used for function names.
    pub function: Style,
    /// Style used for keywords.
    pub keyword: Style,
    /// Style used for numbers.
    pub number: Style,
    /// Style used for operators.
    pub operator: Style,
    /// Style used for punctuation.
    pub punctuation: Style,
    /// Style used for strings.
    pub string: Style,
    /// Style used for types.
    pub type_: Style,
    /// Style used for variables.
    pub variable: Style,
}

impl SyntaxStyles {
    /// Creates a new set of fully resolved syntax styles.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        comment: Style,
        constant: Style,
        function: Style,
        keyword: Style,
        number: Style,
        operator: Style,
        punctuation: Style,
        string: Style,
        type_: Style,
        variable: Style,
    ) -> Self {
        Self {
            comment,
            constant,
            function,
            keyword,
            number,
            operator,
            punctuation,
            string,
            type_,
            variable,
        }
    }
}

/// A fully resolved theme ready for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Resolved UI styles used by rendering code.
    pub ui: UiStyles,
    /// Resolved syntax styles used by highlighting code.
    pub syntax: SyntaxStyles,
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
        ui: UiStyles,
        syntax: SyntaxStyles,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            default_style,
            ui,
            syntax,
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

    fn theme(name: &str) -> Theme {
        let default_style = Style::new().bold();
        let ui_styles = UiStyles::new(
            Style::new().fg(Color::ansi(1)),
            Style::new().fg(Color::ansi(2)),
            Style::new().fg(Color::ansi(3)),
            Style::new().fg(Color::ansi(4)),
            Style::new().fg(Color::ansi(5)),
            Style::new().fg(Color::ansi(6)),
            Style::new().fg(Color::ansi(7)),
        );
        let syntax_styles = SyntaxStyles::new(
            Style::new().fg(Color::ansi(10)),
            Style::new().fg(Color::ansi(11)),
            Style::new().fg(Color::ansi(12)),
            Style::new().fg(Color::ansi(13)),
            Style::new().fg(Color::ansi(14)),
            Style::new().fg(Color::ansi(15)),
            Style::new().fg(Color::ansi(16)),
            Style::new().fg(Color::ansi(17)),
            Style::new().fg(Color::ansi(18)),
            Style::new().fg(Color::ansi(19)),
        );

        Theme::new(
            name,
            ThemeKind::Ansi256,
            default_style,
            ui_styles,
            syntax_styles,
        )
    }

    #[test]
    fn theme_returns_predefined_ui_styles() {
        let theme = theme("demo");

        assert_eq!(theme.ui.status_bar, Style::new().fg(Color::ansi(1)));
        assert_eq!(theme.ui.modified_marker, Style::new().fg(Color::ansi(2)));
        assert_eq!(theme.ui.window, Style::new().fg(Color::ansi(7)));
    }

    #[test]
    fn theme_returns_predefined_syntax_styles() {
        let theme = theme("demo");

        assert_eq!(theme.syntax.comment, Style::new().fg(Color::ansi(10)));
        assert_eq!(theme.syntax.variable, Style::new().fg(Color::ansi(19)));
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
