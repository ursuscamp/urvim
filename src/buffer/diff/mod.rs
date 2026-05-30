use super::{BufferId, LineEdit};
use crate::path::AbsolutePath;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// A normalized line-based diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffHunk {
    /// Kind of change represented by the hunk.
    pub kind: DiffMarkerKind,
    /// First changed line in the current buffer snapshot.
    pub start_line: usize,
    /// End of the changed range, exclusive.
    pub end_line: usize,
}

/// Kind of diff marker shown in the gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffMarkerKind {
    /// Lines added in the working tree.
    Added,
    /// Lines removed from the working tree.
    Deleted,
    /// Lines modified in place.
    Modified,
}

impl DiffHunk {
    /// Creates a new diff hunk.
    pub fn new(start_line: usize, end_line: usize) -> Self {
        Self {
            kind: DiffMarkerKind::Modified,
            start_line,
            end_line,
        }
    }

    /// Creates an added hunk.
    pub fn added(start_line: usize, end_line: usize) -> Self {
        Self {
            kind: DiffMarkerKind::Added,
            start_line,
            end_line,
        }
    }

    /// Creates a deleted hunk.
    pub fn deleted(start_line: usize) -> Self {
        Self {
            kind: DiffMarkerKind::Deleted,
            start_line,
            end_line: start_line,
        }
    }

    /// Creates a modified hunk.
    pub fn modified(start_line: usize, end_line: usize) -> Self {
        Self {
            kind: DiffMarkerKind::Modified,
            start_line,
            end_line,
        }
    }

    fn is_empty(self) -> bool {
        self.start_line == self.end_line
    }

    fn contains_line(self, line: usize) -> bool {
        if self.is_empty() {
            return self.start_line == line;
        }

        line >= self.start_line && line < self.end_line
    }
}

/// A snapshot of diff data produced by a provider.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffSnapshot {
    /// Whether the source file is tracked by the backing VCS provider.
    pub tracked: bool,
    /// Normalized diff hunks in buffer order.
    pub hunks: Vec<DiffHunk>,
}

impl DiffSnapshot {
    /// Creates an empty snapshot for an untracked file.
    pub fn untracked() -> Self {
        Self {
            tracked: false,
            hunks: Vec::new(),
        }
    }
}

/// Input snapshot used by a diff provider.
#[derive(Debug)]
pub struct DiffInput<'a> {
    /// Resolved buffer path, if one exists.
    pub path: Option<&'a AbsolutePath>,
    /// Current buffer lines without trailing newlines.
    pub lines: &'a [String],
}

/// Computes diff snapshots for a buffer.
pub trait DiffProvider {
    /// Collects diff hunks for the given input.
    fn collect(&self, input: &DiffInput<'_>) -> io::Result<DiffSnapshot>;
}

/// Git-backed diff provider that reports unstaged changes.
#[derive(Debug, Default, Clone, Copy)]
pub struct GitDiffProvider;

impl DiffProvider for GitDiffProvider {
    fn collect(&self, input: &DiffInput<'_>) -> io::Result<DiffSnapshot> {
        let Some(path) = input.path else {
            return Ok(DiffSnapshot::untracked());
        };

        let path = path.as_path().canonicalize()?;

        let Some(repo_root) = Self::repo_root(path.as_path())? else {
            return Ok(DiffSnapshot::untracked());
        };

        let relative_path = match path.as_path().strip_prefix(&repo_root) {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => return Ok(DiffSnapshot::untracked()),
        };

        let index_text = match Self::git_show_blob(&repo_root, &relative_path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(DiffSnapshot::untracked());
            }
            Err(error) => return Err(error),
        };
        let current_text = if input.lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", input.lines.join("\n"))
        };
        let hunks = Self::diff_texts(&repo_root, &index_text, &current_text)?;
        Ok(DiffSnapshot {
            tracked: true,
            hunks,
        })
    }
}

impl GitDiffProvider {
    fn repo_root(path: &Path) -> io::Result<Option<PathBuf>> {
        let Some(parent) = path.parent() else {
            return Ok(None);
        };

        let output = Command::new("git")
            .arg("-C")
            .arg(parent)
            .arg("rev-parse")
            .arg("--show-toplevel")
            .output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if root.is_empty() {
            return Ok(None);
        }

        Ok(Some(std::fs::canonicalize(PathBuf::from(root))?))
    }

    fn git_show_blob(root: &Path, relative_path: &Path) -> io::Result<String> {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(root)
            .arg("show")
            .arg(format!(":{}", relative_path.display()));

        let output = command.output()?;
        if !output.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "failed to read git blob",
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn diff_texts(root: &Path, base_text: &str, current_text: &str) -> io::Result<Vec<DiffHunk>> {
        let temp_dir = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let base_path = temp_dir.join(format!("urvim-diff-{pid}-{stamp}-base"));
        let current_path = temp_dir.join(format!("urvim-diff-{pid}-{stamp}-current"));

        fs::write(&base_path, base_text)?;
        fs::write(&current_path, current_text)?;

        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("diff")
            .arg("--no-index")
            .arg("--unified=0")
            .arg("--no-ext-diff")
            .arg("--no-color")
            .arg("--")
            .arg(&base_path)
            .arg(&current_path)
            .output();

        fs::remove_file(&base_path).ok();
        fs::remove_file(&current_path).ok();

        let output = output?;
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(io::Error::new(io::ErrorKind::Other, "git diff failed"));
        }

        let hunks = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(parse_unified_diff_hunk)
            .collect::<Vec<_>>();
        Ok(merge_hunks(hunks))
    }
}

fn parse_unified_diff_hunk(line: &str) -> Option<DiffHunk> {
    let mut parts = line.split_whitespace();
    let header = parts.next()?;
    if header != "@@" {
        return None;
    }
    let old_range = parts.next()?;
    let new_range = parts.next()?;
    let (_, old_len) = parse_range(old_range.strip_prefix('-')?)?;
    let (new_start, new_len) = parse_range(new_range.strip_prefix('+')?)?;
    let start_line = new_start.saturating_sub(1);
    let end_line = if new_len == 0 {
        start_line
    } else {
        start_line.saturating_add(new_len)
    };
    let kind = if old_len == 0 {
        DiffMarkerKind::Added
    } else if new_len == 0 {
        DiffMarkerKind::Deleted
    } else {
        DiffMarkerKind::Modified
    };
    Some(DiffHunk {
        kind,
        start_line,
        end_line,
    })
}

fn parse_range(range: &str) -> Option<(usize, usize)> {
    let mut pieces = range.split(',');
    let start_line = pieces.next()?.parse::<usize>().ok()?;
    let len = pieces
        .next()
        .map_or(1, |len| len.parse::<usize>().ok().unwrap_or(1));
    Some((start_line, len))
}

fn merge_hunks(mut hunks: Vec<DiffHunk>) -> Vec<DiffHunk> {
    hunks.sort_by_key(|hunk| (hunk.start_line, hunk.end_line));
    let mut merged: Vec<DiffHunk> = Vec::with_capacity(hunks.len());

    for hunk in hunks {
        if let Some(last) = merged.last_mut()
            && hunk.start_line <= last.end_line
        {
            last.end_line = last.end_line.max(hunk.end_line);
            if last.kind != hunk.kind {
                last.kind = DiffMarkerKind::Modified;
            }
            continue;
        }
        merged.push(hunk);
    }

    merged
}

/// Cached diff state maintained on a buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffCache {
    hunks: Vec<DiffHunk>,
}

impl DiffCache {
    /// Creates an empty diff cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the cached hunks in buffer order.
    pub fn hunks(&self) -> &[DiffHunk] {
        &self.hunks
    }

    /// Returns true when no hunks are cached.
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    /// Replaces all cached hunks.
    pub fn replace_hunks(&mut self, hunks: Vec<DiffHunk>) {
        self.hunks = merge_hunks(hunks);
    }

    /// Clears the cached hunks.
    pub fn clear(&mut self) {
        self.hunks.clear();
    }

    /// Applies one normalized line edit to the cache.
    pub fn apply_edit(&mut self, edit: LineEdit) {
        self.apply_edits(&[edit]);
    }

    /// Applies a batch of normalized line edits to the cache.
    pub fn apply_edits(&mut self, edits: &[LineEdit]) {
        if edits.is_empty() {
            return;
        }

        for edit in edits {
            if edit.line_delta == 0 {
                continue;
            }

            for hunk in &mut self.hunks {
                if hunk.is_empty() {
                    if hunk.start_line < edit.start_line {
                        continue;
                    }
                } else if hunk.end_line <= edit.start_line {
                    continue;
                }

                if hunk.start_line > edit.start_line {
                    hunk.start_line = hunk.start_line.saturating_add_signed(edit.line_delta);
                    hunk.end_line = hunk.end_line.saturating_add_signed(edit.line_delta);
                    continue;
                }

                hunk.end_line = hunk.end_line.saturating_add_signed(edit.line_delta);
                if hunk.end_line < hunk.start_line {
                    hunk.end_line = hunk.start_line;
                }
            }
        }

        self.hunks = merge_hunks(self.hunks.clone());
    }

    /// Returns true when the given line intersects a cached hunk.
    pub fn line_is_changed(&self, line: usize) -> bool {
        self.hunks.iter().any(|hunk| hunk.contains_line(line))
    }

    /// Returns a marker for each visible row.
    pub fn markers_for_visible_rows(
        &self,
        start_line: usize,
        visible_rows: usize,
    ) -> Vec<Option<DiffMarkerKind>> {
        (0..visible_rows)
            .map(|row| {
                self.hunks.iter().find_map(|hunk| {
                    if hunk.contains_line(start_line + row) {
                        Some(hunk.kind)
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Returns the next hunk start after the provided cursor line.
    pub fn next_hunk_start_line(&self, line: usize) -> Option<usize> {
        let reference = self
            .hunks
            .iter()
            .find(|hunk| hunk.contains_line(line))
            .map(|hunk| {
                if hunk.is_empty() {
                    hunk.start_line.saturating_add(1)
                } else {
                    hunk.end_line
                }
            })
            .unwrap_or_else(|| line.saturating_add(1));

        let idx = self
            .hunks
            .partition_point(|hunk| hunk.start_line < reference);
        self.hunks.get(idx).map(|hunk| hunk.start_line)
    }

    /// Returns the previous hunk start before the provided cursor line.
    pub fn previous_hunk_start_line(&self, line: usize) -> Option<usize> {
        let reference = self
            .hunks
            .iter()
            .find(|hunk| hunk.contains_line(line))
            .map(|hunk| hunk.start_line)
            .unwrap_or(line);

        let idx = self
            .hunks
            .partition_point(|hunk| hunk.start_line < reference);
        idx.checked_sub(1)
            .and_then(|idx| self.hunks.get(idx).map(|hunk| hunk.start_line))
    }

    fn hunk_containing_line(&self, line: usize) -> Option<&DiffHunk> {
        self.hunks.iter().find(|hunk| hunk.contains_line(line))
    }

    fn hunk_end_line(hunk: &DiffHunk) -> usize {
        if hunk.is_empty() {
            hunk.start_line
        } else {
            hunk.end_line.saturating_sub(1)
        }
    }

    /// Returns the next hunk start, including the current hunk when the cursor is not on its first line.
    pub fn next_hunk_start_line_including_current(&self, line: usize) -> Option<usize> {
        if let Some(hunk) = self.hunk_containing_line(line) {
            if line > hunk.start_line {
                return Some(hunk.start_line);
            }
            let reference = hunk.end_line.max(line.saturating_add(1));
            let idx = self
                .hunks
                .partition_point(|hunk| hunk.start_line < reference);
            return self.hunks.get(idx).map(|hunk| hunk.start_line);
        }

        let reference = line.saturating_add(1);

        let idx = self
            .hunks
            .partition_point(|hunk| hunk.start_line < reference);
        self.hunks.get(idx).map(|hunk| hunk.start_line)
    }

    /// Returns the previous hunk start, including the current hunk when the cursor is not on its first line.
    pub fn previous_hunk_start_line_including_current(&self, line: usize) -> Option<usize> {
        if let Some(hunk) = self.hunk_containing_line(line) {
            if line > hunk.start_line {
                return Some(hunk.start_line);
            }
        }

        let reference = self
            .hunk_containing_line(line)
            .map(|hunk| hunk.start_line)
            .unwrap_or(line);

        let idx = self
            .hunks
            .partition_point(|hunk| hunk.start_line < reference);
        idx.checked_sub(1)
            .and_then(|idx| self.hunks.get(idx).map(|hunk| hunk.start_line))
    }

    /// Returns the next hunk end, including the current hunk when the cursor is not on its last line.
    pub fn next_hunk_end_line_including_current(&self, line: usize) -> Option<usize> {
        if let Some(hunk) = self.hunk_containing_line(line) {
            let current_end = Self::hunk_end_line(hunk);
            if line < current_end {
                return Some(current_end);
            }
            let reference = hunk.end_line.max(line.saturating_add(1));
            let idx = self
                .hunks
                .partition_point(|hunk| hunk.start_line < reference);
            return self.hunks.get(idx).map(Self::hunk_end_line);
        }

        let reference = line.saturating_add(1);

        let idx = self
            .hunks
            .partition_point(|hunk| hunk.start_line < reference);
        self.hunks.get(idx).map(Self::hunk_end_line)
    }

    /// Returns the previous hunk end, including the current hunk when the cursor is not on its last line.
    pub fn previous_hunk_end_line_including_current(&self, line: usize) -> Option<usize> {
        if let Some(hunk) = self.hunk_containing_line(line) {
            let current_end = Self::hunk_end_line(hunk);
            if line < current_end {
                return Some(current_end);
            }
        }

        let reference = self
            .hunk_containing_line(line)
            .map(|hunk| hunk.start_line)
            .unwrap_or(line);

        let idx = self
            .hunks
            .partition_point(|hunk| hunk.start_line < reference);
        idx.checked_sub(1)
            .and_then(|idx| self.hunks.get(idx).map(Self::hunk_end_line))
    }
}

/// Result produced by a background diff refresh job.
#[derive(Debug, Clone)]
pub struct DiffRefreshResult {
    /// Buffer receiving the refreshed diff data.
    pub buffer_id: BufferId,
    /// Generation used when the refresh was requested.
    pub generation: u64,
    /// Whether the file is tracked.
    pub tracked: bool,
    /// Normalized diff hunks.
    pub hunks: Vec<DiffHunk>,
}

/// Background job that refreshes a buffer's diff cache.
#[derive(Debug)]
pub struct DiffRefreshJob {
    buffer_id: BufferId,
    generation: u64,
    path: AbsolutePath,
    lines: Vec<String>,
}

impl DiffRefreshJob {
    /// Creates a new diff refresh job.
    pub fn new(
        buffer_id: BufferId,
        generation: u64,
        path: AbsolutePath,
        lines: Vec<String>,
    ) -> Self {
        Self {
            buffer_id,
            generation,
            path,
            lines,
        }
    }

    /// Runs the diff refresh job on a worker thread.
    pub fn run(
        self,
        context: &crate::background::JobContext,
        event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    ) {
        let provider = GitDiffProvider;
        let input = DiffInput {
            path: Some(&self.path),
            lines: &self.lines,
        };

        let snapshot = provider
            .collect(&input)
            .unwrap_or_else(|_| DiffSnapshot::untracked());
        event_tx
            .send(crate::background::JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: Some(crate::background::JobPayload::DiffRefresh(
                    DiffRefreshResult {
                        buffer_id: self.buffer_id,
                        generation: self.generation,
                        tracked: snapshot.tracked,
                        hunks: snapshot.hunks,
                    },
                )),
            })
            .ok();
    }
}

#[cfg(test)]
mod tests;
