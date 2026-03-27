//! Theme loading and registry errors.

use std::fmt;

/// Errors that can occur while loading or registering themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeLoadError {
    /// A theme document could not be parsed from TOML.
    Parse { source: String, message: String },
    /// A theme name was seen more than once.
    DuplicateThemeName(String),
    /// A theme name or key was invalid for the current operation.
    InvalidThemeName(String),
    /// A requested theme name could not be found.
    UnknownThemeName(String),
    /// A required theme section was missing.
    MissingSection {
        theme: String,
        section: &'static str,
    },
    /// A palette entry used an invalid color value.
    InvalidPaletteValue {
        theme: String,
        key: String,
        value: String,
    },
    /// A style referenced a palette entry that does not exist.
    UnknownPaletteReference {
        theme: String,
        section: &'static str,
        key: String,
        reference: String,
    },
}

impl fmt::Display for ThemeLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { source, message } => {
                write!(f, "failed to parse theme {source}: {message}")
            }
            Self::DuplicateThemeName(name) => write!(f, "duplicate theme name: {name}"),
            Self::InvalidThemeName(name) => write!(f, "invalid theme name: {name}"),
            Self::UnknownThemeName(name) => write!(f, "unknown theme name: {name}"),
            Self::MissingSection { theme, section } => {
                write!(f, "theme {theme} is missing required section [{section}]")
            }
            Self::InvalidPaletteValue { theme, key, value } => {
                write!(
                    f,
                    "theme {theme} has invalid palette value {value:?} for {key}"
                )
            }
            Self::UnknownPaletteReference {
                theme,
                section,
                key,
                reference,
            } => {
                write!(
                    f,
                    "theme {theme} has unknown palette reference {reference:?} in {section}.{key}"
                )
            }
        }
    }
}

impl std::error::Error for ThemeLoadError {}
