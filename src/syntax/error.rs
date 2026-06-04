use std::fmt;

/// Errors that can occur while loading syntax definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxLoadError {
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
    /// A syntax region used an invalid regular expression.
    InvalidRegex {
        syntax: String,
        pattern: String,
        message: String,
    },
    /// A syntax declared an invalid comment prefix.
    InvalidCommentPrefix {
        syntax: String,
        comment_prefix: String,
    },
    /// A syntax declared an invalid glyph.
    InvalidGlyph { syntax: String, glyph: String },
    /// A syntax declared an invalid glyph color.
    InvalidGlyphColor { syntax: String, color: String },
}

impl fmt::Display for SyntaxLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::InvalidCommentPrefix {
                syntax,
                comment_prefix,
            } => {
                write!(
                    f,
                    "syntax {syntax} has invalid comment prefix {comment_prefix:?}"
                )
            }
            Self::InvalidGlyph { syntax, glyph } => {
                write!(f, "syntax {syntax} has invalid glyph {glyph:?}")
            }
            Self::InvalidGlyphColor { syntax, color } => {
                write!(f, "syntax {syntax} has invalid glyph color {color:?}")
            }
        }
    }
}

impl std::error::Error for SyntaxLoadError {}
