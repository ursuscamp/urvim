//! User-facing command parsing and resolution.

mod catalog;
mod error;
pub mod highlight;
mod parser;
mod registry;
mod token;

pub use error::CommandError;
pub use highlight::{CommandHighlightKind, CommandHighlightSpan, highlight};
pub use parser::CommandInvocation;

use crate::ui::Intent;

/// Parses a command line into an executable intent.
pub fn parse(input: &str) -> Result<Intent, CommandError> {
    let invocation = parser::parse(input)?;
    let invocation = registry::expand(&invocation)?;
    catalog::resolve(&invocation)
}

/// Returns true when `name` is a protected canonical command root.
pub fn is_canonical_command_root(name: &str) -> bool {
    registry::is_canonical_root(name)
}

/// Validates that an alias expansion parses into at least one command token.
pub fn validate_alias_expansion(input: &str) -> Result<(), CommandError> {
    parser::parse(input).map(|_| ())
}

/// Parses an alias expansion into command tokens.
pub fn parse_alias_expansion(input: &str) -> Result<Vec<String>, CommandError> {
    parser::parse(input).map(|invocation| invocation.tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::editor::ModeKind;
    use crate::globals;
    use crate::ui::Command;
    use std::collections::BTreeMap;

    #[test]
    fn parse_supports_named_and_positional_arguments() {
        assert_eq!(
            parser::parse(r#"buffer write path="notes/today file.txt""#)
                .expect("command should parse")
                .tokens,
            vec!["buffer", "write", "path=notes/today file.txt"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn resolve_write_aliases() {
        assert!(matches!(
            parse("write notes.txt").expect("write should resolve"),
            Intent::Command(Command::SaveBufferAs(_))
        ));
        assert!(matches!(
            parse("buffer write path=notes.txt").expect("buffer write should resolve"),
            Intent::Command(Command::SaveBufferAs(_))
        ));
        assert!(matches!(
            parse("buffer edit").expect("buffer edit should resolve"),
            Intent::Command(Command::OpenUnnamedBuffer)
        ));
        assert!(matches!(
            parse("quit").expect("quit should resolve"),
            Intent::Command(Command::Quit)
        ));
    }

    #[test]
    fn resolve_mode_and_cursor_commands() {
        assert!(matches!(
            parse("mode insert").expect("mode should resolve"),
            Intent::Action(action) if action.to_mode == Some(ModeKind::Insert)
        ));
        assert!(matches!(
            parse("action cursor left count=2").expect("cursor should resolve"),
            Intent::Action(_)
        ));
    }

    #[test]
    fn resolve_edit_operator_and_surround_commands() {
        assert!(matches!(
            parse("action edit delete-line 3").expect("edit action should resolve"),
            Intent::Action(action)
                if matches!(action.kind.as_ref(), Some(crate::editor::ActionKind::Count(3, _)))
        ));
        assert!(matches!(
            parse("action operator delete target=word").expect("operator should resolve"),
            Intent::Action(_)
        ));
        assert!(matches!(
            parse("action tab next").expect("tab navigation should resolve"),
            Intent::Action(_)
        ));
        assert!(matches!(
            parse("action jump backward").expect("jumplist navigation should resolve"),
            Intent::Action(_)
        ));
        assert!(matches!(
            parse("action surround add target=word delimiter=paren")
                .expect("surround should resolve"),
            Intent::Action(_)
        ));
    }

    #[test]
    fn resolve_configured_aliases() {
        let _guard = globals::set_test_config(Config {
            aliases: BTreeMap::from([(
                "dl".to_string(),
                vec![
                    "action".to_string(),
                    "edit".to_string(),
                    "delete-line".to_string(),
                ],
            )]),
            ..Config::default()
        });

        assert!(matches!(
            parse("dl count=2").expect("configured alias should resolve"),
            Intent::Action(action)
                if matches!(action.kind.as_ref(), Some(crate::editor::ActionKind::Count(2, _)))
        ));
    }
}
