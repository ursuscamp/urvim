//! Theme document parsing.

use super::{RawTheme, ThemeLoadError};

/// Parses a TOML theme document into the raw schema model.
pub fn parse_theme(source: &str, input: &str) -> Result<RawTheme, ThemeLoadError> {
    toml::from_str::<RawTheme>(input).map_err(|error| ThemeLoadError::Parse {
        source: source.to_string(),
        message: error.to_string(),
    })
}
