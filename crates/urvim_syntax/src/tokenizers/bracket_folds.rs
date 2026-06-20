//! Shared fold event helpers for brace-like syntaxes.

use crate::state::{SyntaxFoldEvent, SyntaxFoldEventKind, SyntaxFoldKind};

const FOLD_BRACE: SyntaxFoldKind = 1;
const FOLD_BRACKET: SyntaxFoldKind = 2;
const FOLD_PAREN: SyntaxFoldKind = 3;

/// Emits a fold event for one structural delimiter byte.
pub fn push_delimiter_fold_event(fold_events: &mut Vec<SyntaxFoldEvent>, byte: u8) {
    let Some((kind, fold_kind)) = delimiter_fold(byte) else {
        return;
    };

    fold_events.push(SyntaxFoldEvent::new(kind, fold_kind));
}

fn delimiter_fold(byte: u8) -> Option<(SyntaxFoldEventKind, SyntaxFoldKind)> {
    match byte {
        b'{' => Some((SyntaxFoldEventKind::Open, FOLD_BRACE)),
        b'}' => Some((SyntaxFoldEventKind::Close, FOLD_BRACE)),
        b'[' => Some((SyntaxFoldEventKind::Open, FOLD_BRACKET)),
        b']' => Some((SyntaxFoldEventKind::Close, FOLD_BRACKET)),
        b'(' => Some((SyntaxFoldEventKind::Open, FOLD_PAREN)),
        b')' => Some((SyntaxFoldEventKind::Close, FOLD_PAREN)),
        _ => None,
    }
}
