//! Builtin handwritten scanner for Haskell syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{match_operator_from_sets, run_while};
use crate::state::{CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState};
use urvim_theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(COMMENT_LINE, "comment.line");
tag_static!(COMMENT_BLOCK, "comment.block");
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(OP, "operator");

const HASKELL_PRAGMAS: ContextId = ContextId::new("haskell", "haskell_pragmas");
const HASKELL_BLOCK_COMMENT: ContextId = ContextId::new("haskell", "haskell_block_comment");
const HASKELL_STRING: ContextId = ContextId::new("haskell", "haskell_string");

/// Tokenize one line of Haskell using the builtin scanner.
pub(crate) fn tokenize_haskell_line(
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let (mut ctx, inj, parent_style, tokenizer_state) = match state {
        SyntaxState::Code(CodeState::Scanner {
            contexts,
            injection,
            parent_style,
            tokenizer_state,
        }) => (contexts, injection, parent_style, tokenizer_state),
        SyntaxState::Plain => (ContextStack::default(), None, None, Default::default()),
    };

    let mut spans: Vec<SyntaxSpan> = Vec::new();
    let mut index = 0usize;
    let bytes = line.as_bytes();
    let line_len = bytes.len();

    while index < line_len {
        let tail = &line[index..];
        let tail_bytes = &bytes[index..];

        // ── Inside pragma ────────────────────────────────────────────
        if ctx.top_is(HASKELL_PRAGMAS) {
            // Rule 2: #-} close
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'#'
                && tail_bytes[1] == b'-'
                && tail_bytes[2] == b'}'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*KW).clone()));
                ctx.pop_top(HASKELL_PRAGMAS);
                index = end;
                continue;
            }
            // Rule 3: {-# nested open
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'{'
                && tail_bytes[1] == b'-'
                && tail_bytes[2] == b'#'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*KW).clone()));
                ctx.push(HASKELL_PRAGMAS);
                index = end;
                continue;
            }
            // Fall through to top-level rules (pragma content gets
            // highlighted by keyword/type/etc.)
        }

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(HASKELL_BLOCK_COMMENT) {
            // Rule 4: -} close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'-' && tail_bytes[1] == b'}' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(HASKELL_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Rule 5: {- nested open
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'{' && tail_bytes[1] == b'-' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(HASKELL_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Fall through to top-level rules (no content rule for block
            // comments in the TOML definition)
        }

        // ── Inside string ────────────────────────────────────────────
        if ctx.top_is(HASKELL_STRING) {
            // Rule 6: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(HASKELL_STRING);
                index = end;
                continue;
            }
            // Rule 9: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 8: Content [^"\\\n]+
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Top-level ────────────────────────────────────────────────
        // (also reached as fallthrough from pragma / comment contexts)

        // Rule 1: Line comment --
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'-' && tail_bytes[1] == b'-' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        // Rule 3: Pragma open {-# (outside pragma)
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'{'
            && tail_bytes[1] == b'-'
            && tail_bytes[2] == b'#'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*KW).clone()));
            ctx.push(HASKELL_PRAGMAS);
            index = end;
            continue;
        }

        // Rule 5: Block comment open {- (outside comment)
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'{' && tail_bytes[1] == b'-' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(HASKELL_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // Rule 7: String open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(HASKELL_STRING);
            index = end;
            continue;
        }

        // Rule 10: Char literal '...'
        if let Some(ch_len) = match_char_literal(tail) {
            spans.push(SyntaxSpan::new(index, index + ch_len, (*CNST).clone()));
            index += ch_len;
            continue;
        }

        // Rule 11: Keyword with word boundaries
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
            continue;
        }

        // Rule 12: Type (capitalized identifier)
        if let Some(typ_len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + typ_len, (*TYP).clone()));
            index += typ_len;
            continue;
        }

        // Rule 13: Constant with word boundaries (False / True)
        if let Some(cnst_len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + cnst_len, (*CNST).clone()));
            index += cnst_len;
            continue;
        }

        // Rule 14: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 15: Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // Rule 16: Operator
        if let Some(op_len) = match_operator(tail) {
            spans.push(SyntaxSpan::new(index, index + op_len, (*OP).clone()));
            index += op_len;
            continue;
        }

        // ── No match – skip one char ─────────────────────────────────
        let Some(ch) = tail.chars().next() else { break };
        index += ch.len_utf8();
    }

    (
        spans,
        SyntaxState::Code(CodeState::Scanner {
            contexts: ctx,
            injection: inj,
            parent_style,
            tokenizer_state,
        }),
    )
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\''
}

fn match_char_literal(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'\'' {
        return None;
    }
    if bytes[1] == b'\\' {
        if bytes.len() >= 4 && bytes[3] == b'\'' {
            return Some(4);
        }
        None
    } else if bytes[1] != b'\'' && bytes[1] != b'\n' {
        if bytes[2] == b'\'' {
            return Some(3);
        }
        None
    } else {
        None
    }
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &[
        "case", "class", "data", "default", "deriving", "do", "else", "false", "forall", "if",
        "import", "in", "infix", "infixl", "infixr", "instance", "let", "module", "newtype", "of",
        "then", "true", "type", "where",
    ] {
        if tail.starts_with(kw) {
            let after = kw.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }

    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    let bytes = tail.as_bytes();
    if !bytes[0].is_ascii_uppercase() {
        return None;
    }

    let mut i = 1;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'\'')
    {
        i += 1;
    }

    if i < bytes.len() && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for word in &["False", "True"] {
        if tail.starts_with(word) {
            let after = word.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }

    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    if i >= len || !bytes[i].is_ascii_digit() {
        return None;
    }
    i += 1;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }

    if i < len && bytes[i] == b'.' {
        let dot_pos = i;
        i += 1;
        if i < len && bytes[i].is_ascii_digit() {
            i += 1;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            i = dot_pos;
        }
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["::", "->", "<-", "=>", "=", "||", "&&"],
        b"+-*/%=&|!<>^~?:.",
    )
}
