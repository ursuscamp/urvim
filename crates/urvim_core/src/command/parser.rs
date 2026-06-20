use super::CommandError;
use super::token::{TokenizeMode, tokenize};

/// Raw, tokenized command line input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    /// Whitespace-delimited tokens after quote handling.
    pub tokens: Vec<String>,
}

/// Tokenizes a command line into an invocation.
pub fn parse(input: &str) -> Result<CommandInvocation, CommandError> {
    let tokens = tokenize(input, TokenizeMode::Strict)?;
    if tokens.is_empty() {
        return Err(CommandError::Empty);
    }

    Ok(CommandInvocation {
        tokens: tokens.into_iter().map(|token| token.value).collect(),
    })
}
