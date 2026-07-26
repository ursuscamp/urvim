use super::*;
use regex::{Regex, RegexBuilder};

/// Direction used for selecting and navigating search matches.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    /// Select matches after the cursor and navigate toward the end of the buffer.
    #[default]
    Forward,
    /// Select matches before the cursor and navigate toward the start of the buffer.
    Reverse,
}

impl SearchDirection {
    /// Returns the opposite search direction.
    pub const fn opposite(self) -> Self {
        match self {
            Self::Forward => Self::Reverse,
            Self::Reverse => Self::Forward,
        }
    }
}

/// Options applied to an active search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    direction: SearchDirection,
    case_sensitive: bool,
    regex: bool,
}

impl SearchOptions {
    /// Creates search options with the given direction, case, and regex behavior.
    pub const fn new(direction: SearchDirection, case_sensitive: bool, regex: bool) -> Self {
        Self {
            direction,
            case_sensitive,
            regex,
        }
    }

    /// Returns the direction followed by normal search navigation.
    pub const fn direction(self) -> SearchDirection {
        self.direction
    }

    /// Returns whether literal matching distinguishes letter case.
    pub const fn case_sensitive(self) -> bool {
        self.case_sensitive
    }

    /// Returns whether the query is interpreted as a regular expression.
    pub const fn regex(self) -> bool {
        self.regex
    }

    /// Changes the direction followed by normal search navigation.
    pub fn set_direction(&mut self, direction: SearchDirection) {
        self.direction = direction;
    }

    /// Changes whether literal matching distinguishes letter case.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
    }

    /// Changes whether the query is interpreted as a regular expression.
    pub fn set_regex(&mut self, regex: bool) {
        self.regex = regex;
    }
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self::new(SearchDirection::Forward, true, false)
    }
}

/// A compiled, line-oriented search pattern.
#[derive(Debug, Clone)]
pub struct SearchPattern {
    regex: Regex,
    expand_captures: bool,
    empty: bool,
}

impl SearchPattern {
    /// Compiles a literal or regular-expression query using the selected options.
    pub fn compile(query: &str, options: SearchOptions) -> Result<Self, regex::Error> {
        let source = if options.regex() {
            query.to_string()
        } else {
            regex::escape(query)
        };
        let regex = RegexBuilder::new(&source)
            .case_insensitive(!options.case_sensitive())
            .build()?;
        Ok(Self {
            regex,
            expand_captures: options.regex(),
            empty: query.is_empty() || query.contains(['\n', '\r']),
        })
    }
}

impl Buffer {
    /// Returns every non-overlapping literal match in buffer order.
    pub fn find_literal_matches(&self, query: &str, case_sensitive: bool) -> Vec<TextObjectRange> {
        let options = SearchOptions::new(SearchDirection::Forward, case_sensitive, false);
        self.find_search_matches(query, options).unwrap_or_default()
    }

    /// Returns every non-overlapping search match in buffer order.
    pub fn find_search_matches(
        &self,
        query: &str,
        options: SearchOptions,
    ) -> Result<Vec<TextObjectRange>, regex::Error> {
        let pattern = SearchPattern::compile(query, options)?;
        Ok(self.search_matches_for_pattern(&pattern))
    }

    /// Returns search ranges paired with their expanded replacement text.
    pub fn find_search_replacements(
        &self,
        query: &str,
        replacement: &str,
        options: SearchOptions,
    ) -> Result<Vec<(TextObjectRange, String)>, regex::Error> {
        let pattern = SearchPattern::compile(query, options)?;
        if pattern.empty {
            return Ok(Vec::new());
        }
        let mut replacements = Vec::new();
        for line_idx in 0..self.line_count() {
            let Some(line) = self.line_at(line_idx) else {
                continue;
            };
            let mut scratch = String::new();
            let text = line.contiguous_text_with_scratch(&mut scratch);
            for captures in pattern.regex.captures_iter(text) {
                let matched = captures.get(0).expect("capture zero always exists");
                let range = TextObjectRange {
                    start: Cursor::new(line_idx, matched.start()),
                    end: Cursor::new(line_idx, matched.end()),
                };
                let expanded = if pattern.expand_captures {
                    let mut expanded = String::new();
                    captures.expand(replacement, &mut expanded);
                    expanded
                } else {
                    replacement.to_string()
                };
                replacements.push((range, expanded));
            }
        }
        Ok(replacements)
    }

    fn search_matches_for_pattern(&self, pattern: &SearchPattern) -> Vec<TextObjectRange> {
        if pattern.empty {
            return Vec::new();
        }
        let mut matches = Vec::new();
        for line_idx in 0..self.line_count() {
            let Some(line) = self.line_at(line_idx) else {
                continue;
            };
            let mut scratch = String::new();
            let text = line.contiguous_text_with_scratch(&mut scratch);
            matches.extend(
                pattern
                    .regex
                    .find_iter(text)
                    .map(|matched| TextObjectRange {
                        start: Cursor::new(line_idx, matched.start()),
                        end: Cursor::new(line_idx, matched.end()),
                    }),
            );
        }
        matches
    }

    pub fn find_char_forward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let start_col = cursor.col + 1;
        let mut occurrences: Vec<usize> = Vec::new();
        for grapheme in line.graphemes() {
            if grapheme.byte_idx() >= start_col && grapheme.as_str().starts_with(target) {
                occurrences.push(grapheme.byte_idx());
            }
        }
        let target_idx = occurrences.get(count.saturating_sub(1))?;
        Some(Cursor::new(line_idx, *target_idx))
    }

    pub fn find_char_backward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let occurrences: Vec<usize> = line
            .graphemes()
            .filter(|grapheme| {
                grapheme.byte_idx() < cursor.col && grapheme.as_str().starts_with(target)
            })
            .map(|grapheme| grapheme.byte_idx())
            .collect();
        let target_idx = occurrences.len().saturating_sub(count);
        let idx = *occurrences.get(target_idx)?;
        Some(Cursor::new(line_idx, idx))
    }

    /// Moves to the character just before the next forward match.
    pub fn find_till_forward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let search_cursor = self.till_forward_search_cursor(cursor);
        let found = self.find_char_forward(search_cursor, target, count)?;
        Some(self.prev_cursor_line(found).unwrap_or(found))
    }

    /// Moves to the character just after the next backward match.
    pub fn find_till_backward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let search_cursor = Cursor::new(cursor.line, cursor.col.saturating_sub(1));
        let found = self.find_char_backward(search_cursor, target, count)?;
        Some(
            self.next_cursor_line(found)
                .unwrap_or_else(|| Cursor::new(found.line, self.line_len(found.line))),
        )
    }

    fn till_forward_search_cursor(&self, cursor: Cursor) -> Cursor {
        let Some(line) = self.line_at(cursor.line) else {
            return cursor;
        };
        if cursor.col == 0 {
            return Cursor::new(cursor.line, 0);
        }

        let mut col = cursor.col;
        for grapheme in line.graphemes() {
            if grapheme.byte_idx() >= cursor.col {
                col = grapheme.byte_idx() + grapheme.len();
                break;
            }
        }
        Cursor::new(cursor.line, col)
    }
}

#[cfg(test)]
mod literal_tests {
    use super::*;

    #[test]
    fn literal_matches_are_case_sensitive_and_non_overlapping() {
        let buffer = Buffer::from_str("aaa Aa\naaa");
        assert_eq!(
            buffer.find_literal_matches("aa", true),
            vec![
                TextObjectRange {
                    start: Cursor::new(0, 0),
                    end: Cursor::new(0, 2)
                },
                TextObjectRange {
                    start: Cursor::new(1, 0),
                    end: Cursor::new(1, 2)
                },
            ]
        );
        assert!(buffer.find_literal_matches("AA", true).is_empty());
    }

    #[test]
    fn case_insensitive_literal_matches_escape_regex_syntax() {
        let buffer = Buffer::from_str("Foo[1] foo.1 FOO[1]");
        assert_eq!(
            buffer.find_literal_matches("foo[1]", false),
            vec![
                TextObjectRange {
                    start: Cursor::new(0, 0),
                    end: Cursor::new(0, 6),
                },
                TextObjectRange {
                    start: Cursor::new(0, 13),
                    end: Cursor::new(0, 19),
                },
            ]
        );
    }

    #[test]
    fn case_insensitive_literal_matches_use_unicode_byte_ranges() {
        let buffer = Buffer::from_str("Ä ä");
        assert_eq!(
            buffer.find_literal_matches("ä", false),
            vec![
                TextObjectRange {
                    start: Cursor::new(0, 0),
                    end: Cursor::new(0, 2),
                },
                TextObjectRange {
                    start: Cursor::new(0, 3),
                    end: Cursor::new(0, 5),
                },
            ]
        );
    }

    #[test]
    fn literal_matches_use_utf8_byte_columns() {
        let buffer = Buffer::from_str("é猫é猫");
        assert_eq!(
            buffer.find_literal_matches("猫", true),
            vec![
                TextObjectRange {
                    start: Cursor::new(0, 2),
                    end: Cursor::new(0, 5)
                },
                TextObjectRange {
                    start: Cursor::new(0, 7),
                    end: Cursor::new(0, 10)
                },
            ]
        );
    }

    #[test]
    fn empty_and_multiline_queries_do_not_match() {
        let buffer = Buffer::from_str("one\ntwo");
        assert!(buffer.find_literal_matches("", true).is_empty());
        assert!(buffer.find_literal_matches("one\ntwo", false).is_empty());
    }

    #[test]
    fn regex_matches_are_line_oriented_and_support_zero_width_ranges() {
        let buffer = Buffer::from_str("one\ntwo");
        let options = SearchOptions::new(SearchDirection::Forward, true, true);

        assert_eq!(
            buffer.find_search_matches("^", options).unwrap(),
            vec![
                TextObjectRange {
                    start: Cursor::new(0, 0),
                    end: Cursor::new(0, 0),
                },
                TextObjectRange {
                    start: Cursor::new(1, 0),
                    end: Cursor::new(1, 0),
                },
            ]
        );
        assert!(
            buffer
                .find_search_matches("one\\ntwo", options)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn regex_matching_honors_case_sensitivity() {
        let buffer = Buffer::from_str("One one");
        let insensitive = SearchOptions::new(SearchDirection::Forward, false, true);
        let sensitive = SearchOptions::new(SearchDirection::Forward, true, true);

        assert_eq!(
            buffer
                .find_search_matches("one", insensitive)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            buffer.find_search_matches("one", sensitive).unwrap().len(),
            1
        );
    }

    #[test]
    fn regex_replacements_expand_numbered_named_and_literal_dollar_references() {
        let buffer = Buffer::from_str("Ada Lovelace");
        let options = SearchOptions::new(SearchDirection::Forward, true, true);
        let replacements = buffer
            .find_search_replacements(
                r"(?<first>\w+) (\w+)",
                "${2}, $first ($0) $$${missing}",
                options,
            )
            .unwrap();

        assert_eq!(replacements.len(), 1);
        assert_eq!(replacements[0].1, "Lovelace, Ada (Ada Lovelace) $");
    }

    #[test]
    fn literal_replacements_do_not_expand_capture_references() {
        let buffer = Buffer::from_str("one");
        let replacements = buffer
            .find_search_replacements("one", "$1 $$", SearchOptions::default())
            .unwrap();

        assert_eq!(replacements[0].1, "$1 $$");
    }

    #[test]
    fn invalid_regex_is_reported() {
        let options = SearchOptions::new(SearchDirection::Forward, true, true);
        assert!(SearchPattern::compile("(", options).is_err());
    }
}
