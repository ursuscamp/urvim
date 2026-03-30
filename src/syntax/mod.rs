//! Syntax registry, schema, and loader for urvim syntax highlighting.

mod builtin;
mod definition;
mod error;
mod loader;
mod normalize;
mod registry;

pub use definition::{
    ContextControl, ContextEntry, ContextMatch, ContextPush, InjectedSyntaxFallback,
    InjectedSyntaxRule, InjectedSyntaxSelector, SyntaxDefinition, SyntaxMetadata, SyntaxRule,
};
pub use error::SyntaxLoadError;
pub use registry::{
    SyntaxRegistry, builtin_syntax_registry, fallback_syntax_name, resolve_builtin_syntax,
};
