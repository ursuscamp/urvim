//! Theme system primitives for urvim.
//!
//! This module defines the unified theme schema used by the editor, including
//! hierarchical highlight names, raw TOML-facing models, resolved theme data
//! structures, and the registry/error types that loading code builds on.

mod error;
mod loader;
mod model;
mod parser;
mod schema;
mod tag;

pub use error::ThemeLoadError;
pub use loader::{resolve_theme, resolve_theme_from_str};
pub use model::{HighlightStyles, StyleOverlay, Theme, ThemeKind, ThemeRegistry};
pub use parser::parse_theme;
pub use schema::{RawColorValue, RawStyle, RawTheme};
pub use tag::TagParents;
pub use tag::{Tag, TagError};
