//! Shared delimiter-pair definitions used by insert-mode pairing behavior.

/// Supported delimiter pair definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    /// Opening delimiter.
    pub opening: char,
    /// Closing delimiter.
    pub closing: char,
}

const PAIRS: [Pair; 6] = [
    Pair {
        opening: '(',
        closing: ')',
    },
    Pair {
        opening: '[',
        closing: ']',
    },
    Pair {
        opening: '{',
        closing: '}',
    },
    Pair {
        opening: '"',
        closing: '"',
    },
    Pair {
        opening: '\'',
        closing: '\'',
    },
    Pair {
        opening: '`',
        closing: '`',
    },
];

/// Returns the matching closer for a supported opener.
pub fn closer_for(opening: char) -> Option<char> {
    PAIRS
        .iter()
        .find(|pair| pair.opening == opening)
        .map(|pair| pair.closing)
}

/// Returns the matching opener for a supported closer.
pub fn opener_for(closing: char) -> Option<char> {
    PAIRS
        .iter()
        .find(|pair| pair.closing == closing)
        .map(|pair| pair.opening)
}

/// Returns true when the character is one of the supported openers.
pub fn is_supported_opener(ch: char) -> bool {
    closer_for(ch).is_some()
}

/// Returns true when the character is one of the supported closers.
pub fn is_supported_closer(ch: char) -> bool {
    opener_for(ch).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opener_lookup() {
        assert_eq!(closer_for('('), Some(')'));
        assert_eq!(closer_for('['), Some(']'));
        assert_eq!(closer_for('{'), Some('}'));
        assert_eq!(closer_for('"'), Some('"'));
        assert_eq!(closer_for('\''), Some('\''));
        assert_eq!(closer_for('`'), Some('`'));
        assert_eq!(closer_for('<'), None);
    }

    #[test]
    fn test_closer_lookup() {
        assert_eq!(opener_for(')'), Some('('));
        assert_eq!(opener_for(']'), Some('['));
        assert_eq!(opener_for('}'), Some('{'));
        assert_eq!(opener_for('"'), Some('"'));
        assert_eq!(opener_for('\''), Some('\''));
        assert_eq!(opener_for('`'), Some('`'));
        assert_eq!(opener_for('>'), None);
    }

    #[test]
    fn test_supported_pairs_checks() {
        assert!(is_supported_opener('('));
        assert!(is_supported_closer(')'));
        assert!(!is_supported_opener('<'));
        assert!(!is_supported_closer('>'));
    }
}
