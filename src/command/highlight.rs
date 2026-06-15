use std::ops::Range;

use super::registry;
use super::token::{CommandToken, TokenizeMode, tokenize};

/// A highlighted byte range in a command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHighlightSpan {
    /// Byte range in the original input.
    pub range: Range<usize>,
    /// Semantic highlight kind.
    pub kind: CommandHighlightKind,
}

impl CommandHighlightSpan {
    fn new(range: Range<usize>, kind: CommandHighlightKind) -> Self {
        Self { range, kind }
    }
}

/// Highlight categories for command-line syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandHighlightKind {
    /// Exact command root.
    Command,
    /// Prefix of a valid command root.
    Prefix,
    /// Positional token after the root command.
    Subcommand,
    /// Argument name in `name=value` syntax.
    Name,
    /// String-like argument value.
    String,
    /// Numeric argument value.
    Number,
    /// Constant/keyword-like value.
    Constant,
    /// Operator token such as `=`.
    Operator,
    /// Invalid root command.
    Error,
}

impl CommandHighlightKind {
    /// Returns the theme tag name used for this kind.
    pub fn theme_name(self) -> &'static str {
        match self {
            CommandHighlightKind::Command => "function",
            CommandHighlightKind::Prefix => "variable",
            CommandHighlightKind::Subcommand => "keyword",
            CommandHighlightKind::Name => "constant",
            CommandHighlightKind::String => "string",
            CommandHighlightKind::Number => "number",
            CommandHighlightKind::Constant => "constant",
            CommandHighlightKind::Operator => "operator",
            CommandHighlightKind::Error => "ui.notification.error",
        }
    }
}

/// Produces permissive syntax highlighting for a command line.
pub fn highlight(input: &str) -> Vec<CommandHighlightSpan> {
    let tokens = tokenize(input, TokenizeMode::Permissive).unwrap_or_default();
    let Some(root) = tokens.first().cloned() else {
        return Vec::new();
    };

    let mut spans = Vec::new();
    match root_status(&root.value) {
        MatchStatus::Exact => spans.push(CommandHighlightSpan::new(
            root.raw.clone(),
            CommandHighlightKind::Command,
        )),
        MatchStatus::Prefix => {
            spans.push(CommandHighlightSpan::new(
                root.raw.clone(),
                CommandHighlightKind::Prefix,
            ));
            return spans;
        }
        MatchStatus::Unknown => {
            spans.push(CommandHighlightSpan::new(
                root.raw.clone(),
                CommandHighlightKind::Error,
            ));
            return spans;
        }
    }

    for token in tokens.into_iter().skip(1) {
        highlight_token(token, &mut spans);
    }

    spans
}

fn highlight_token(token: CommandToken, spans: &mut Vec<CommandHighlightSpan>) {
    if let Some((_name, value, eq_idx)) = split_named_arg(token.value.as_str()) {
        spans.push(CommandHighlightSpan::new(
            token.raw.start..token.raw.start + eq_idx,
            CommandHighlightKind::Name,
        ));
        spans.push(CommandHighlightSpan::new(
            token.raw.start + eq_idx..token.raw.start + eq_idx + 1,
            CommandHighlightKind::Operator,
        ));

        if value.is_empty() {
            return;
        }

        let value_start = token.raw.start + eq_idx + 1;
        let kind = if token.quoted {
            CommandHighlightKind::String
        } else if value.chars().all(|ch| ch.is_ascii_digit()) {
            CommandHighlightKind::Number
        } else {
            CommandHighlightKind::Constant
        };
        spans.push(CommandHighlightSpan::new(value_start..token.raw.end, kind));
    } else {
        spans.push(CommandHighlightSpan::new(
            token.raw,
            CommandHighlightKind::Subcommand,
        ));
    }
}

fn split_named_arg(text: &str) -> Option<(&str, &str, usize)> {
    let (name, value) = text.split_once('=')?;
    Some((name, value, name.len()))
}

fn root_status(text: &str) -> MatchStatus {
    if registry::is_registered_root(text) {
        MatchStatus::Exact
    } else if registry::is_registered_root_prefix(text) {
        MatchStatus::Prefix
    } else {
        MatchStatus::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchStatus {
    Exact,
    Prefix,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simplify(spans: Vec<CommandHighlightSpan>) -> Vec<(usize, usize, CommandHighlightKind)> {
        spans
            .into_iter()
            .map(|span| (span.range.start, span.range.end, span.kind))
            .collect()
    }

    #[test]
    fn highlights_root_command_and_positional_token() {
        assert_eq!(
            simplify(highlight("buffer cursor")),
            vec![
                (0, 6, CommandHighlightKind::Command),
                (7, 13, CommandHighlightKind::Subcommand)
            ]
        );
    }

    #[test]
    fn highlights_registered_alias_roots() {
        assert_eq!(
            simplify(highlight("cursor left")),
            vec![
                (0, 6, CommandHighlightKind::Command),
                (7, 11, CommandHighlightKind::Subcommand),
            ]
        );
    }

    #[test]
    fn highlights_configured_alias_roots() {
        let _guard = crate::globals::set_test_config(crate::config::Config {
            aliases: std::collections::BTreeMap::from([(
                "dl".to_string(),
                vec![
                    "action".to_string(),
                    "edit".to_string(),
                    "delete-line".to_string(),
                ],
            )]),
            ..crate::config::Config::default()
        });

        assert_eq!(
            simplify(highlight("dl count=2")),
            vec![
                (0, 2, CommandHighlightKind::Command),
                (3, 8, CommandHighlightKind::Name),
                (8, 9, CommandHighlightKind::Operator),
                (9, 10, CommandHighlightKind::Number),
            ]
        );
    }

    #[test]
    fn highlights_prefix_only_for_partial_root() {
        assert_eq!(
            simplify(highlight("wri")),
            vec![(0, 3, CommandHighlightKind::Prefix)]
        );
    }

    #[test]
    fn highlights_named_arguments_syntactically() {
        assert_eq!(
            simplify(highlight("write path=notes.txt count=2")),
            vec![
                (0, 5, CommandHighlightKind::Command),
                (6, 10, CommandHighlightKind::Name),
                (10, 11, CommandHighlightKind::Operator),
                (11, 20, CommandHighlightKind::Constant),
                (21, 26, CommandHighlightKind::Name),
                (26, 27, CommandHighlightKind::Operator),
                (27, 28, CommandHighlightKind::Number),
            ]
        );
    }

    #[test]
    fn does_not_error_on_extra_tokens() {
        assert_eq!(
            simplify(highlight("write-all extra stuff")),
            vec![
                (0, 9, CommandHighlightKind::Command),
                (10, 15, CommandHighlightKind::Subcommand),
                (16, 21, CommandHighlightKind::Subcommand),
            ]
        );
    }

    #[test]
    fn unknown_root_is_still_an_error() {
        assert_eq!(
            simplify(highlight("save file")),
            vec![(0, 4, CommandHighlightKind::Error)]
        );
    }
}
