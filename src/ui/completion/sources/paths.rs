//! Path completion source.

use super::{PathPrefixKind, current_path_prefix};
use crate::buffer::{Buffer, Cursor, TextRef};
use crate::ui::completion::CompletionCandidate;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub fn path_completion_candidates(
    buffer: &Buffer,
    cursor: Cursor,
    cwd: Option<&Path>,
    home: Option<&Path>,
) -> Vec<CompletionCandidate> {
    let Some((start, prefix_kind, prefix)) = current_path_prefix(buffer, cursor) else {
        return Vec::new();
    };

    let Some((root, display_prefix)) =
        path_completion_scope(prefix_kind, prefix.as_str(), cwd, home)
    else {
        return Vec::new();
    };

    let entries = match collect_paths(root.as_path(), display_prefix.as_str()) {
        Some(entries) => entries,
        None => return Vec::new(),
    };

    let query = prefix.to_lowercase();
    let matches: Vec<_> = entries
        .into_iter()
        .filter(|candidate| candidate.to_lowercase().starts_with(query.as_str()))
        .collect();

    matches
        .into_iter()
        .map(|path| {
            let symbol = path_completion_symbol_for_path(Path::new(&path));
            CompletionCandidate::new(
                path.clone(),
                path,
                crate::buffer::TextObjectRange {
                    start,
                    end: Cursor::new(
                        cursor.line,
                        cursor.col.min(
                            buffer
                                .line_at(cursor.line)
                                .map(|line| line.len())
                                .unwrap_or(0),
                        ),
                    ),
                },
                symbol,
            )
        })
        .collect()
}

fn path_completion_scope(
    prefix_kind: PathPrefixKind,
    prefix: &str,
    cwd: Option<&Path>,
    home: Option<&Path>,
) -> Option<(PathBuf, String)> {
    let (base_root, prefix_head, raw_path) = match prefix_kind {
        PathPrefixKind::Absolute => (PathBuf::from("/"), "/", prefix.strip_prefix('/')?),
        PathPrefixKind::CurrentDir => (cwd?.to_path_buf(), "./", prefix.strip_prefix("./")?),
        PathPrefixKind::ParentDir => (
            cwd?.parent()?.to_path_buf(),
            "../",
            prefix.strip_prefix("../")?,
        ),
        PathPrefixKind::HomeDir => (home?.to_path_buf(), "~/", prefix.strip_prefix("~/")?),
    };

    let (dir_part, _) = raw_path.rsplit_once('/').unwrap_or(("", raw_path));
    let root = if dir_part.is_empty() {
        base_root
    } else {
        base_root.join(dir_part)
    };

    let display_prefix = if prefix.ends_with('/') {
        prefix.to_string()
    } else if dir_part.is_empty() {
        prefix_head.to_string()
    } else {
        format!("{}{}{}", prefix_head, dir_part, "/")
    };

    Some((root, display_prefix))
}

fn collect_paths(root: &Path, display_prefix: &str) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    let mut builder = WalkBuilder::new(root);
    let walker = builder.max_depth(Some(1));
    for entry in walker.build().flatten() {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        if rel.as_os_str().is_empty() {
            continue;
        }

        let candidate = if display_prefix.is_empty() {
            root.join(rel).to_string_lossy().into_owned()
        } else {
            let relative = rel.to_string_lossy();
            format!("{}{}", display_prefix, relative)
        };
        paths.push(candidate.clone());
    }

    paths.sort_by_key(|path| path.to_lowercase());
    paths.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Some(paths)
}

fn path_completion_symbol_for_path(path: &Path) -> Option<String> {
    if let Some(glyph) = crate::syntax::FiletypeGlyph::from_path(path) {
        return Some(format!("{} ", glyph.glyph));
    }

    if crate::globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false) {
        Some(" ".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::config::{AdvancedGlyphCapability, Config};
    use crate::globals;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn path_completion_ignores_non_path_prefixes() {
        let buffer = Buffer::from_str("foo/bar");
        let cwd = temp_dir_path("path-ignore");

        assert!(
            path_completion_candidates(&buffer, Cursor::new(0, 7), Some(cwd.as_path()), None)
                .is_empty()
        );
    }

    #[test]
    fn path_completion_returns_first_level_cwd_matches() {
        let cwd = temp_dir_path("path-relative");
        std::fs::create_dir_all(cwd.join("src").join("nested")).unwrap();
        std::fs::write(cwd.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(
            cwd.join("src").join("nested").join("deeper.rs"),
            "fn main() {}\n",
        )
        .unwrap();

        let buffer = Buffer::from_str("./src/");
        let labels: Vec<_> =
            path_completion_candidates(&buffer, Cursor::new(0, 6), Some(cwd.as_path()), None)
                .into_iter()
                .map(|candidate| candidate.label)
                .collect();

        assert!(labels.contains(&"./src/main.rs".to_string()));
        assert!(labels.contains(&"./src/nested".to_string()));
        assert!(!labels.contains(&"./src/nested/deeper.rs".to_string()));
    }

    #[test]
    fn path_completion_supports_parent_and_home_prefixes() {
        let parent = temp_dir_path("path-parent-base");
        let cwd = parent.join("cwd");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(parent.join("outside.txt"), "outside\n").unwrap();

        let home = temp_dir_path("path-home");
        std::fs::write(home.join("home.txt"), "home\n").unwrap();

        let parent_buffer = Buffer::from_str("../out");
        let parent_labels: Vec<_> = path_completion_candidates(
            &parent_buffer,
            Cursor::new(0, 6),
            Some(cwd.as_path()),
            Some(home.as_path()),
        )
        .into_iter()
        .map(|candidate| candidate.label)
        .collect();

        let home_buffer = Buffer::from_str("~/ho");
        let home_labels: Vec<_> = path_completion_candidates(
            &home_buffer,
            Cursor::new(0, 4),
            Some(cwd.as_path()),
            Some(home.as_path()),
        )
        .into_iter()
        .map(|candidate| candidate.label)
        .collect();

        assert_eq!(parent_labels, vec!["../outside.txt".to_string()]);
        assert_eq!(home_labels, vec!["~/home.txt".to_string()]);
    }

    #[test]
    fn path_completion_uses_filetype_glyph_when_nerdfonts_are_enabled() {
        let _guard = globals::set_test_config(Config {
            advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        let cwd = temp_dir_path("path-icon");
        std::fs::write(cwd.join("icon.rs"), "fn main() {}\n").unwrap();

        let buffer = Buffer::from_str("./ic");
        let candidates =
            path_completion_candidates(&buffer, Cursor::new(0, 4), Some(cwd.as_path()), None);
        let symbol = candidates
            .first()
            .and_then(|candidate| candidate.symbol.as_deref());

        assert_eq!(symbol, Some(" "));
    }

    fn temp_dir_path(name: &str) -> PathBuf {
        let unique = format!(
            "urvim-completion-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
