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
    /// The unified map of hierarchical highlight names.
    #[serde(default)]
    pub highlights: BTreeMap<String, RawStyle>,
}

/// A raw style definition used by the default and highlight sections.
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
    /// Whether this raw style should overlay a blank style instead of the theme default.
    #[serde(rename = "overlay", default)]
    pub overlay: bool,
}
