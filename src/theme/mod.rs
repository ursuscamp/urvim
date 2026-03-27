//! Theme system primitives for urvim.
//!
//! This module defines the closed theme schema used by the editor, including
//! the predefined UI and syntax keys, raw TOML-facing models, resolved theme
//! data structures, and the registry/error types that future loading code will
//! build on.

mod error;
mod keys;
mod loader;
mod model;
mod parser;
mod schema;

pub use error::ThemeLoadError;
pub use keys::{SyntaxStyleKey, UiStyleKey};
pub use loader::{resolve_theme, resolve_theme_from_str};
pub use model::{StyleOverride, SyntaxStyles, Theme, ThemeKind, ThemeRegistry, UiStyles};
pub use parser::parse_theme;
pub use schema::{RawColorValue, RawStyle, RawSyntaxStyles, RawTheme, RawUiStyles};
