use super::*;
use crate::globals;
use crate::ui::picker::PickerSource;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use urvim_terminal::Color;
use urvim_terminal::Style;
use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

#[test]
fn git_picker_collects_untracked_files_and_multiple_hunks() {
    let repo = temp_repo();
    let tracked = repo.join("tracked.txt");
    fs::write(&tracked, "one\ntwo\nthree\nfour\nfive\n").expect("write tracked file");
    git(&repo, ["add", "tracked.txt"]);
    git(
        &repo,
        [
            "-c",
            "user.name=urvim",
            "-c",
            "user.email=urvim@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ],
    );

    fs::write(&tracked, "one\nTWO\nthree\nFOUR\nfive\n").expect("modify tracked file");
    let untracked = repo.join("new.txt");
    fs::write(&untracked, "hello\n").expect("write untracked file");

    let items = collect_git_picker_items(repo.as_path()).expect("collect git picker items");
    assert_eq!(items.len(), 3);

    let tracked_items = items
        .iter()
        .filter(|item| item.path.file_name().and_then(|name| name.to_str()) == Some("tracked.txt"))
        .collect::<Vec<_>>();
    assert_eq!(tracked_items.len(), 2);

    let hunk_lines = tracked_items
        .iter()
        .map(|item| item.formatted_line(Style::default()).render_segments(120))
        .collect::<Vec<_>>();
    assert_eq!(hunk_lines[0][0].text, ".M");
    assert!(
        hunk_lines[0]
            .iter()
            .any(|segment| segment.text.contains(":2"))
    );
    assert!(
        hunk_lines[1]
            .iter()
            .any(|segment| segment.text.contains(":4"))
    );

    let untracked_item = items
        .iter()
        .find(|item| item.path.file_name().and_then(|name| name.to_str()) == Some("new.txt"))
        .expect("untracked item");
    let rendered = untracked_item
        .formatted_line(Style::default())
        .render_segments(120);
    assert_eq!(rendered[0].text, "??");
}

#[test]
fn git_picker_status_styles_follow_theme() {
    let _theme_guard = globals::set_test_active_theme(test_theme());

    let base_style = Style::default();
    let item = GitPickerItem {
        path: PathBuf::from("tracked.txt"),
        root: PathBuf::from("."),
        status: GitStatus {
            index: ' ',
            worktree: 'M',
        },
        hunk: None,
    };

    let rendered = item.formatted_line(base_style).render_segments(120);
    let expected = base_style
        .accent(theme_style("ui.window.gutter.diff.modified"))
        .bold();
    assert_eq!(rendered[0].style, expected);
}

#[test]
fn git_picker_emits_stage_and_discard_intents() {
    let source = GitPickerSource::new(PathBuf::from("/tmp"));
    let item = GitPickerItem {
        path: PathBuf::from("/tmp/example.txt"),
        root: PathBuf::from("/tmp"),
        status: GitStatus {
            index: ' ',
            worktree: 'M',
        },
        hunk: None,
    };

    assert!(matches!(
        source.stage_intent(&item),
        Some(crate::ui::Intent::Command(
            crate::ui::Command::GitPickerToggleStage(_)
        ))
    ));
    assert!(matches!(
        source.discard_intent(&item),
        Some(crate::ui::Intent::Command(
            crate::ui::Command::GitPickerDiscard(_)
        ))
    ));
}

fn temp_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let repo = std::env::temp_dir().join(format!(
        "urvim-git-picker-repo-{}-{}",
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&repo).expect("create temp repo");
    git(&repo, ["init", "-q"]);
    repo
}

fn git<const N: usize>(dir: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("run git command");
    assert!(status.success(), "git command failed");
}

fn test_theme() -> Theme {
    let mut highlights = HighlightStyles::default();
    highlights.insert(
        Tag::parse("ui.window.gutter.diff.added").expect("valid tag"),
        Style::new().fg(Color::ansi(24)),
    );
    highlights.insert(
        Tag::parse("ui.window.gutter.diff.deleted").expect("valid tag"),
        Style::new().fg(Color::ansi(25)),
    );
    highlights.insert(
        Tag::parse("ui.window.gutter.diff.modified").expect("valid tag"),
        Style::new().fg(Color::ansi(26)),
    );

    Theme::new("test", ThemeKind::Ansi256, Style::default(), highlights)
}

fn theme_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}
