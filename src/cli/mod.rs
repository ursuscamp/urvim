//! CLI argument parsing helpers.

use crate::buffer::Cursor;
use std::path::PathBuf;
use std::str::FromStr;

/// A CLI file argument plus an optional initial cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliFileSpec {
    /// File path to open.
    pub path: PathBuf,
    /// Optional zero-based cursor converted from the 1-based CLI suffix.
    pub cursor: Option<Cursor>,
}

impl FromStr for CliFileSpec {
    type Err = std::convert::Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(parse_cli_file_spec(input))
    }
}

/// Parses a CLI file argument, preserving colon-containing paths when the suffix is not numeric.
///
/// The `:line[:column]` suffix is 1-based on input and converted to the buffer's
/// zero-based `Cursor` coordinates.
pub fn parse_cli_file_spec(input: &str) -> CliFileSpec {
    let segments = input.split(':').collect::<Vec<_>>();
    if segments.len() < 2 {
        return CliFileSpec {
            path: PathBuf::from(input),
            cursor: None,
        };
    }

    let Some(last) = segments
        .last()
        .and_then(|segment| segment.parse::<usize>().ok())
    else {
        return CliFileSpec {
            path: PathBuf::from(input),
            cursor: None,
        };
    };

    if segments.len() >= 3 {
        if let Some(line) = segments
            .get(segments.len() - 2)
            .and_then(|segment| segment.parse::<usize>().ok())
        {
            let path = segments[..segments.len() - 2].join(":");
            return CliFileSpec {
                path: PathBuf::from(path),
                cursor: Some(Cursor::new(line.saturating_sub(1), last.saturating_sub(1))),
            };
        }
    }

    let path = segments[..segments.len() - 1].join(":");
    CliFileSpec {
        path: PathBuf::from(path),
        cursor: Some(Cursor::new(last.saturating_sub(1), 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_path_only_arguments() {
        let spec = parse_cli_file_spec("/tmp/file.txt");
        assert_eq!(spec.path, PathBuf::from("/tmp/file.txt"));
        assert_eq!(spec.cursor, None);
    }

    #[test]
    fn parses_line_only_suffix() {
        let spec = parse_cli_file_spec("/tmp/file.txt:12");
        assert_eq!(spec.path, PathBuf::from("/tmp/file.txt"));
        assert_eq!(spec.cursor, Some(Cursor::new(11, 0)));
    }

    #[test]
    fn parses_line_and_column_suffix() {
        let spec = parse_cli_file_spec("/tmp/file.txt:12:4");
        assert_eq!(spec.path, PathBuf::from("/tmp/file.txt"));
        assert_eq!(spec.cursor, Some(Cursor::new(11, 3)));
    }

    #[test]
    fn preserves_colon_paths_when_suffix_is_not_numeric() {
        let spec = parse_cli_file_spec("/tmp/with:colons/file.txt:abc");
        assert_eq!(spec.path, PathBuf::from("/tmp/with:colons/file.txt:abc"));
        assert_eq!(spec.cursor, None);
    }

    #[test]
    fn preserves_path_prefixes_before_numeric_suffixes() {
        let spec = parse_cli_file_spec("/tmp/with:colons/file.txt:8");
        assert_eq!(spec.path, PathBuf::from("/tmp/with:colons/file.txt"));
        assert_eq!(spec.cursor, Some(Cursor::new(7, 0)));
    }
}
