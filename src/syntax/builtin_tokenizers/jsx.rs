//! Shared JSX/TSX matching helpers for JavaScript-family tokenizers.

/// A JSX tag match split into delimiter and tag-name spans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsxTagMatch {
    /// Byte length of the matched opening fragment or tag opener.
    pub len: usize,
    /// Byte range for the tag name within the match.
    pub name: Option<(usize, usize)>,
    /// Whether the match includes a closing slash after `<`.
    pub has_slash: bool,
}

/// Match a JSX tag opener/name starting at `<`, including fragments.
pub fn match_jsx_tag(tail: &str) -> Option<JsxTagMatch> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if len < 2 || bytes[0] != b'<' {
        return None;
    }

    if bytes[1] == b'>' {
        return Some(JsxTagMatch {
            len: 2,
            name: None,
            has_slash: false,
        });
    }
    if len >= 3 && bytes[1] == b'/' && bytes[2] == b'>' {
        return Some(JsxTagMatch {
            len: 3,
            name: None,
            has_slash: true,
        });
    }

    let mut i = 1;
    let mut has_slash = false;
    if bytes[i] == b'/' {
        has_slash = true;
        i += 1;
    }
    if i >= len || !is_jsx_ident_start(bytes[i]) {
        return None;
    }

    let name_start = i;
    i += 1;
    while i < len && is_jsx_ident_part(bytes[i]) {
        i += 1;
    }
    Some(JsxTagMatch {
        len: i,
        name: Some((name_start, i)),
        has_slash,
    })
}

/// Match a JSX attribute name. This intentionally accepts boolean attributes.
pub fn match_jsx_attribute(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.is_empty() || !is_jsx_ident_start(bytes[0]) {
        return None;
    }

    let mut i = 1;
    while i < bytes.len() && is_jsx_ident_part(bytes[i]) {
        i += 1;
    }

    let mut j = i;
    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
    }

    if j >= bytes.len() || matches!(bytes[j], b'=' | b'/' | b'>' | b'{') {
        return Some(i);
    }
    None
}

fn is_jsx_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
}

fn is_jsx_ident_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'-' | b'.' | b':')
}
