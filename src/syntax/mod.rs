//! Syntax registry and built-in tokenizer metadata for urvim syntax highlighting.

mod builtin;
pub(crate) mod builtin_tokenizers;
mod definition;
mod error;
mod normalize;
mod registry;

pub use definition::{
    SyntaxDefinition, SyntaxMetadata, SyntaxMetadataSpec, SyntaxSpec, SyntaxTokenizer,
};
pub use error::SyntaxLoadError;
pub use registry::{
    SyntaxRegistry, builtin_syntax_registry, fallback_syntax_name, resolve_builtin_syntax,
};
