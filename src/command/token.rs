use std::ops::Range;

use super::CommandError;

/// A token extracted from a command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandToken {
    /// Raw byte range in the original input.
    pub raw: Range<usize>,
    /// Decoded token text with quotes removed and escapes resolved.
    pub value: String,
    /// Whether the token started with a quote.
    pub quoted: bool,
}

/// Tokenization mode for command lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TokenizeMode {
    /// Return an error when a quoted token is not terminated.
    Strict,
    /// Produce best-effort tokens for incomplete input.
    Permissive,
}

/// Tokenizes a command line, optionally permitting incomplete quotes.
pub(super) fn tokenize(input: &str, mode: TokenizeMode) -> Result<Vec<CommandToken>, CommandError> {
    let mut tokens = Vec::new();
    let mut cursor = 0usize;

    while cursor < input.len() {
        let Some((ch, ch_len)) = next_char(input, cursor) else {
            break;
        };
        if is_delimiter(ch) {
            cursor += ch_len;
            continue;
        }

        let start = cursor;
        let mut value = String::new();
        let mut quoted = false;
        let mut in_quote: Option<char> = None;

        while cursor < input.len() {
            let Some((next, next_len)) = next_char(input, cursor) else {
                break;
            };

            if let Some(quote) = in_quote {
                cursor += next_len;
                match next {
                    '\\' => {
                        if let Some((escaped, escaped_len)) = next_char(input, cursor) {
                            value.push(escaped);
                            cursor += escaped_len;
                        }
                    }
                    c if c == quote => {
                        in_quote = None;
                    }
                    c => value.push(c),
                }
                continue;
            }

            if is_delimiter(next) {
                break;
            }

            cursor += next_len;
            if is_quote(next) {
                quoted = true;
                in_quote = Some(next);
            } else {
                value.push(next);
            }
        }

        if in_quote.is_some() && matches!(mode, TokenizeMode::Strict) {
            return Err(CommandError::UnterminatedQuote);
        }

        tokens.push(CommandToken {
            raw: start..cursor,
            value,
            quoted,
        });
    }

    Ok(tokens)
}

fn next_char(input: &str, cursor: usize) -> Option<(char, usize)> {
    let ch = input[cursor..].chars().next()?;
    Some((ch, ch.len_utf8()))
}

fn is_delimiter(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

fn is_quote(ch: char) -> bool {
    matches!(ch, '"' | '\'')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_tokenizes_quoted_strings() {
        let input = r#"write "notes/today file.txt""#;
        let tokens = tokenize(input, TokenizeMode::Strict).expect("tokenization should succeed");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].value, "write");
        assert_eq!(tokens[1].value, "notes/today file.txt");
        assert_eq!(&input[tokens[1].raw.clone()], r#""notes/today file.txt""#);
        assert!(tokens[1].quoted);
    }

    #[test]
    fn strict_tokenizes_escaped_quotes() {
        let input = r#"edit "fo\"o""#;
        let tokens = tokenize(input, TokenizeMode::Strict).expect("tokenization should succeed");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[1].value, "fo\"o");
        assert_eq!(&input[tokens[1].raw.clone()], r#""fo\"o""#);
        assert!(tokens[1].quoted);
    }

    #[test]
    fn strict_rejects_unterminated_quote() {
        let error = tokenize("edit \"foo", TokenizeMode::Strict).expect_err("should fail");
        assert_eq!(error, CommandError::UnterminatedQuote);
    }

    #[test]
    fn permissive_keeps_unterminated_quote_as_token() {
        let tokens =
            tokenize("edit \"foo", TokenizeMode::Permissive).expect("tokenization should succeed");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].value, "edit");
        assert_eq!(tokens[1].value, "foo");
        assert_eq!(tokens[1].raw.start, 5);
        assert_eq!(tokens[1].raw.end, 9);
        assert!(tokens[1].quoted);
    }
}
