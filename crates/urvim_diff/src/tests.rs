use super::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cache_shifts_hunks_with_line_edits() {
    let mut cache = DiffCache::new();
    cache.replace_hunks(vec![DiffHunk::new(1, 3), DiffHunk::new(5, 6)]);

    cache.apply_edit(DiffEdit::new(0, 1));

    assert_eq!(cache.hunks(), &[DiffHunk::new(2, 4), DiffHunk::new(6, 7)]);
}

#[test]
fn cache_navigates_between_hunks() {
    let mut cache = DiffCache::new();
    cache.replace_hunks(vec![DiffHunk::new(1, 2), DiffHunk::new(4, 5)]);

    assert_eq!(cache.next_hunk_start_line(0), Some(1));
    assert_eq!(cache.next_hunk_start_line(1), Some(4));
    assert_eq!(cache.previous_hunk_start_line(4), Some(1));
    assert_eq!(cache.previous_hunk_start_line(1), None);
}

#[test]
fn cache_includes_current_hunk_for_start_and_end_navigation() {
    let mut cache = DiffCache::new();
    cache.replace_hunks(vec![DiffHunk::new(1, 3), DiffHunk::new(4, 5)]);

    assert_eq!(cache.next_hunk_start_line_including_current(2), Some(1));
    assert_eq!(cache.next_hunk_start_line_including_current(1), Some(4));
    assert_eq!(cache.previous_hunk_start_line_including_current(2), Some(1));
    assert_eq!(cache.next_hunk_end_line_including_current(1), Some(2));
    assert_eq!(cache.previous_hunk_end_line_including_current(1), Some(2));
}

#[test]
fn git_provider_reports_tracked_unstaged_changes() {
    let repo = temp_repo();
    let file = repo.join("tracked.txt");
    fs::write(&file, "one\ntwo\nthree\n").expect("write tracked file");
    git(&repo, ["add", "tracked.txt"]);
    fs::write(&file, "one\nTWO\nthree\n").expect("modify tracked file");

    let provider = GitDiffProvider;
    let lines = vec!["one".to_string(), "TWO".to_string(), "three".to_string()];
    let snapshot = provider
        .collect(&DiffInput {
            path: Some(file.as_path()),
            lines: &lines,
        })
        .expect("collect diff snapshot");

    assert!(snapshot.tracked);
    assert_eq!(snapshot.hunks, vec![DiffHunk::new(1, 2)]);
}

#[test]
fn git_provider_ignores_untracked_files() {
    let repo = temp_repo();
    let file = repo.join("untracked.txt");
    fs::write(&file, "hello\nworld\n").expect("write untracked file");

    let provider = GitDiffProvider;
    let lines = vec!["hello".to_string(), "world".to_string()];
    let snapshot = provider
        .collect(&DiffInput {
            path: Some(file.as_path()),
            lines: &lines,
        })
        .expect("collect diff snapshot");

    assert!(!snapshot.tracked);
    assert!(snapshot.hunks.is_empty());
}

fn temp_repo() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let repo =
        std::env::temp_dir().join(format!("urvim-diff-repo-{}-{}", std::process::id(), stamp));
    fs::create_dir_all(&repo).expect("create temp repo");
    git(&repo, ["init", "-q"]);
    repo
}

fn git<const N: usize>(dir: &PathBuf, args: [&str; N]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("run git command");
    assert!(status.success(), "git command failed");
}
