use std::fmt;

/// Errors raised while parsing or resolving a user command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// The input was empty or whitespace only.
    Empty,
    /// A quoted token was never terminated.
    UnterminatedQuote,
    /// No command matched the first token.
    UnknownCommand(String),
    /// Alias expansion exceeded the maximum allowed depth.
    AliasExpansionCycle(String),
    /// A command group matched, but the subcommand did not.
    UnknownSubcommand { command: String, subcommand: String },
    /// A required argument was missing.
    MissingArgument { command: String, name: String },
    /// An argument was provided more than once.
    DuplicateArgument { command: String, name: String },
    /// A positional or named argument could not be parsed.
    InvalidArgument {
        command: String,
        name: String,
        value: String,
        expected: &'static str,
    },
    /// The command received an extra positional argument.
    UnexpectedArgument { command: String, value: String },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "command must not be empty"),
            Self::UnterminatedQuote => write!(f, "command contains an unterminated quote"),
            Self::UnknownCommand(command) => write!(f, "Unknown command: {command}"),
            Self::AliasExpansionCycle(command) => {
                write!(f, "Alias expansion cycle detected for command: {command}")
            }
            Self::UnknownSubcommand {
                command,
                subcommand,
            } => {
                write!(f, "Unknown subcommand for {command}: {subcommand}")
            }
            Self::MissingArgument { command, name } => {
                write!(f, "Missing argument for {command}: {name}")
            }
            Self::DuplicateArgument { command, name } => {
                write!(f, "Duplicate argument for {command}: {name}")
            }
            Self::InvalidArgument {
                command,
                name,
                value,
                expected,
            } => write!(
                f,
                "Invalid argument for {command}: {name}={value} (expected {expected})"
            ),
            Self::UnexpectedArgument { command, value } => {
                write!(f, "Unexpected argument for {command}: {value}")
            }
        }
    }
}

impl std::error::Error for CommandError {}
