use std::fmt;

/// Errors that can occur while loading syntax definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxLoadError {
    /// A syntax document could not be parsed from TOML.
    Parse { source: String, message: String },
    /// A syntax name was empty or invalid.
    InvalidSyntaxName(String),
    /// A syntax alias was empty or invalid.
    InvalidSyntaxAlias { syntax: String, alias: String },
    /// A syntax name was seen more than once.
    DuplicateSyntaxName(String),
    /// A metadata matcher was assigned to more than one syntax definition.
    DuplicateMetadataMapping {
        field: String,
        pattern: String,
        first: String,
        second: String,
    },
    /// A syntax alias was declared more than once for the same syntax.
    DuplicateSyntaxAlias { syntax: String, alias: String },
    /// A syntax referenced an invalid tag.
    InvalidTag { syntax: String, tag: String },
    /// A syntax region referenced an unknown nested syntax.
    UnknownNestedSyntax { syntax: String, nested: String },
    /// A syntax region configured injected syntax without a selector.
    MissingInjectedSyntaxSelector { syntax: String },
    /// A syntax region configured more than one injected syntax selector.
    ConflictingInjectedSyntaxSelector { syntax: String },
    /// A syntax region used an invalid regular expression.
    InvalidRegex {
        syntax: String,
        pattern: String,
        message: String,
    },
    /// A syntax region referenced an invalid context marker.
    InvalidContextMarker { syntax: String, marker: String },
}

impl fmt::Display for SyntaxLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { source, message } => {
                write!(f, "failed to parse syntax {source}: {message}")
            }
            Self::InvalidSyntaxName(name) => write!(f, "invalid syntax name: {name}"),
            Self::InvalidSyntaxAlias { syntax, alias } => {
                write!(f, "syntax {syntax} has invalid alias {alias}")
            }
            Self::DuplicateSyntaxName(name) => write!(f, "duplicate syntax name: {name}"),
            Self::DuplicateMetadataMapping {
                field,
                pattern,
                first,
                second,
            } => {
                write!(
                    f,
                    "{field} pattern {pattern} is mapped to both {first} and {second}"
                )
            }
            Self::DuplicateSyntaxAlias { syntax, alias } => {
                write!(f, "syntax {syntax} declares duplicate alias {alias}")
            }
            Self::InvalidTag { syntax, tag } => {
                write!(f, "syntax {syntax} references invalid tag {tag}")
            }
            Self::UnknownNestedSyntax { syntax, nested } => {
                write!(
                    f,
                    "syntax {syntax} references unknown nested syntax {nested}"
                )
            }
            Self::MissingInjectedSyntaxSelector { syntax } => {
                write!(
                    f,
                    "syntax {syntax} configured injected syntax without a selector"
                )
            }
            Self::ConflictingInjectedSyntaxSelector { syntax } => {
                write!(
                    f,
                    "syntax {syntax} configured more than one injected syntax selector"
                )
            }
            Self::InvalidRegex {
                syntax,
                pattern,
                message,
            } => {
                write!(
                    f,
                    "syntax {syntax} has invalid regex {pattern:?}: {message}"
                )
            }
            Self::InvalidContextMarker { syntax, marker } => {
                write!(f, "syntax {syntax} has invalid context marker {marker}")
            }
        }
    }
}

impl std::error::Error for SyntaxLoadError {}
