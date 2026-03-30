//! Raw theme schema models used for TOML parsing.

use std::collections::BTreeMap;

use serde::Deserialize;

/// A raw color value as it appears in a palette entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum RawColorValue {
    /// An ANSI 256-color palette index.
    Ansi(u8),
    /// A 24-bit RGB color.
    Rgb(String),
}

/// A raw theme document loaded from TOML.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTheme {
    /// The user-facing theme name.
    pub name: String,
    /// Named palette colors used by all other style sections.
    pub palette: BTreeMap<String, RawColorValue>,
    /// The theme default style.
    pub default: RawStyle,
    /// Predefined UI styles.
    pub ui: RawUiStyles,
    /// Syntax tag styles.
    pub syntax: BTreeMap<String, RawStyle>,
}

/// A raw style definition used by the default, UI, and syntax sections.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStyle {
    /// Foreground palette color name.
    pub fg: Option<String>,
    /// Background palette color name.
    pub bg: Option<String>,
    /// Underline color palette name.
    pub underline_color: Option<String>,
    /// Bold flag.
    pub bold: Option<bool>,
    /// Italic flag.
    pub italic: Option<bool>,
    /// Underline flag.
    pub underline: Option<bool>,
    /// Double underline flag.
    pub double_underline: Option<bool>,
    /// Dim flag.
    pub dim: Option<bool>,
    /// Reverse flag.
    pub reverse: Option<bool>,
    /// Blink flag.
    pub blink: Option<bool>,
    /// Strikethrough flag.
    pub strikethrough: Option<bool>,
    /// Overline flag.
    pub overline: Option<bool>,
}

/// Closed raw UI style definitions.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawUiStyles {
    /// The editor status bar.
    pub status_bar: RawStyle,
    /// The modified-buffer marker.
    pub modified_marker: RawStyle,
    /// The currently active tab.
    pub tab_active: RawStyle,
    /// A non-active tab.
    pub tab_inactive: RawStyle,
    /// A scroll indicator shown in the tab bar.
    pub tab_scroll_indicator: RawStyle,
    /// The gutter beside the buffer text.
    pub gutter: RawStyle,
    /// The main buffer viewport.
    pub window: RawStyle,
}
