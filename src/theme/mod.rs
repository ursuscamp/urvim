//! Theme system primitives for urvim.
//!
//! This module defines the theme schema used by the editor, including the
//! predefined UI keys, hierarchical syntax tags, raw TOML-facing models,
//! resolved theme data structures, and the registry/error types that future
//! loading code will build on.

mod error;
mod keys;
mod loader;
mod model;
mod parser;
mod schema;
mod tag;

pub use error::ThemeLoadError;
pub use keys::UiStyleKey;
pub use loader::{resolve_theme, resolve_theme_from_str};
pub use model::{StyleOverlay, SyntaxTagStyles, Theme, ThemeKind, ThemeRegistry, UiStyles};
pub use parser::parse_theme;
pub use schema::{RawColorValue, RawStyle, RawTheme, RawUiStyles};
pub use tag::TagParents;
pub use tag::{Tag, TagError};
