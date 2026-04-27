//! Syntax glyph styling helpers for compact UI surfaces.

use crate::globals;
use crate::terminal::Style;
use smol_str::SmolStr;
use std::path::Path;

use super::{SyntaxMetadata, builtin_syntax_registry};

/// A filetype glyph paired with the style used to render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiletypeGlyph {
    /// Glyph text.
    pub glyph: SmolStr,
    /// Glyph style.
    pub style: Style,
}

impl FiletypeGlyph {
    /// Resolves a filetype glyph for a path using the active glyph configuration.
    pub fn from_path(path: &Path) -> Option<Self> {
        let nerdfont_enabled =
            globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false);
        if !nerdfont_enabled {
            return None;
        }

        let registry = builtin_syntax_registry().ok()?;
        let syntax_name = registry.resolve_for_input(Some(path), None)?;
        let metadata = registry.metadata(syntax_name.as_str())?;
        Self::from_metadata(Some(&metadata), nerdfont_enabled)
    }

    /// Resolves a filetype glyph for syntax metadata, if glyph rendering is enabled.
    pub fn from_metadata(
        metadata: Option<&SyntaxMetadata>,
        nerdfont_enabled: bool,
    ) -> Option<Self> {
        if !nerdfont_enabled {
            return None;
        }

        let metadata = metadata?;
        let glyph = metadata.glyph.clone()?;
        let style = metadata
            .glyph_color
            .map(|color| Style::default().fg(color))
            .unwrap_or_default();

        Some(Self { glyph, style })
    }
}
