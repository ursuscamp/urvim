//! Shared helpers for builtin syntax scanners.

/// Returns the byte length of the longest character run matching `predicate`.
pub fn run_while(input: &str, predicate: fn(char) -> bool) -> usize {
    input
        .char_indices()
        .find(|(_, ch)| !predicate(*ch))
        .map_or(input.len(), |(index, _)| index)
}

/// Returns true when `byte` is an ASCII word character.
pub fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Matches the first word in `words` using caller-provided word boundaries.
pub fn match_word_from_list(
    tail: &str,
    words: &[&str],
    index: usize,
    full_bytes: &[u8],
    is_word: fn(u8) -> bool,
) -> Option<usize> {
    if index > 0 && is_word(full_bytes[index - 1]) {
        return None;
    }

    for word in words {
        if tail.starts_with(word) {
            let after = word.len();
            if after >= tail.len() || !is_word(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }

    None
}

/// Matches ordered multi-byte operators before single-byte operators.
pub fn match_operator_from_sets(tail: &str, multi: &[&str], single: &[u8]) -> Option<usize> {
    for op in multi {
        if tail.starts_with(op) {
            return Some(op.len());
        }
    }

    let first = *tail.as_bytes().first()?;
    single.contains(&first).then_some(1)
}

/// Matches an identifier followed by optional spaces/tabs and an opening paren.
pub fn match_function_call_with(
    tail: &str,
    is_start: fn(u8) -> bool,
    is_continue: fn(u8) -> bool,
) -> Option<usize> {
    let bytes = tail.as_bytes();
    let first = *bytes.first()?;
    if !is_start(first) {
        return None;
    }

    let mut ident_end = 1;
    while ident_end < bytes.len() && is_continue(bytes[ident_end]) {
        ident_end += 1;
    }

    let mut lookahead = ident_end;
    while lookahead < bytes.len() && matches!(bytes[lookahead], b' ' | b'\t') {
        lookahead += 1;
    }

    if lookahead < bytes.len() && bytes[lookahead] == b'(' {
        Some(ident_end)
    } else {
        None
    }
}

/// Matches a required byte prefix followed by an identifier.
pub fn match_prefixed_identifier_with(
    tail: &str,
    prefix: u8,
    is_start: fn(u8) -> bool,
    is_continue: fn(u8) -> bool,
) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != prefix || !is_start(bytes[1]) {
        return None;
    }

    let mut i = 2;
    while i < bytes.len() && is_continue(bytes[i]) {
        i += 1;
    }
    Some(i)
}

/// Matches optional spaces/tabs, then a byte prefix followed by an identifier.
pub fn match_line_prefixed_identifier_with(
    tail: &str,
    prefix: u8,
    is_start: fn(u8) -> bool,
    is_continue: fn(u8) -> bool,
) -> Option<usize> {
    let bytes = tail.as_bytes();
    let mut i = 0;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
        i += 1;
    }

    if i >= bytes.len() || bytes[i] != prefix {
        return None;
    }
    i += 1;

    if i >= bytes.len() || !is_start(bytes[i]) {
        return None;
    }
    i += 1;

    while i < bytes.len() && is_continue(bytes[i]) {
        i += 1;
    }
    Some(i)
}

/// Matches a backslash escape that consumes the escaped byte unchanged.
pub fn match_two_byte_escape(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    (bytes.len() >= 2 && bytes[0] == b'\\').then_some(2)
}
