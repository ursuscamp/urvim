//! Hierarchical syntax tags used by syntax and theme resolution.

use smol_str::SmolStr;
use std::fmt;

/// A validated hierarchical syntax tag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(SmolStr);

/// Errors that can occur while parsing a tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagError {
    input: String,
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid tag: {}", self.input)
    }
}

impl std::error::Error for TagError {}

/// Iterator over the specificity chain of a tag.
pub struct TagParents<'a> {
    next: Option<&'a str>,
}

impl Tag {
    /// Parses a validated tag from text.
    pub fn parse(value: &str) -> Result<Self, TagError> {
        let value = value.trim();
        if !is_valid_tag(value) {
            return Err(TagError {
                input: value.to_string(),
            });
        }

        Ok(Self(SmolStr::new(value)))
    }

    /// Returns the tag as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns parent candidates from most specific to least specific.
    pub fn parent_chain(&self) -> TagParents<'_> {
        TagParents {
            next: Some(self.as_str()),
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> Iterator for TagParents<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = current.rsplit_once('.').map(|(parent, _)| parent);
        Some(current)
    }
}

fn is_valid_tag(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    value.split('.').all(is_valid_segment)
}

fn is_valid_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !first.is_ascii_lowercase() {
        return false;
    }

    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_tags() {
        let tag = Tag::parse("constant.integer").expect("valid tag");
        assert_eq!(tag.as_str(), "constant.integer");
        let chain: Vec<&str> = tag.parent_chain().collect();
        assert_eq!(chain, vec!["constant.integer", "constant"]);
    }

    #[test]
    fn clones_remain_usable() {
        let tag = Tag::parse("string.escape").expect("valid tag");
        let cloned = tag.clone();
        assert_eq!(cloned.as_str(), "string.escape");
        assert_eq!(
            cloned.parent_chain().collect::<Vec<_>>(),
            vec!["string.escape", "string"]
        );
    }

    #[test]
    fn rejects_invalid_tags() {
        for value in [
            "",
            "Constant",
            "constant..integer",
            ".constant",
            "constant.Integer",
        ] {
            assert!(Tag::parse(value).is_err(), "{value} should be rejected");
        }
    }
}
