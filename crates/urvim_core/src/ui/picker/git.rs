//! Git picker source and selection behavior.

use crate::background::JobPayload;
use crate::background::{JobContext, JobManager, JobToken};
use crate::buffer::{Cursor, DiffHunk, merge_hunks, parse_unified_diff_hunk};
use crate::globals;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::line::{
    display_path_relative_to, push_file_glyph, push_fixed_text, push_tail_label,
};
use crate::ui::picker::preview::spawn_preview_loader;
use crate::ui::picker::query::{
    FuzzyMatchScore, PickerQueryMode, exact_matches, fuzzy_match_score, query_prompt_segments,
};
use crate::ui::picker::{
    FormattedLineTemplate, PickerFormattedLine, PickerItem, PickerPreview, PickerPreviewEvent,
    PickerSearchEvent, PickerSource, PickerWidget,
};
use crate::ui::{Command, Intent};
use std::ffi::OsString;
use std::io;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use urvim_terminal::Style;

const PICKER_CHUNK_SIZE: usize = 64;
const GIT_PREVIEW_CONTEXT_LINES: usize = 100;
static NEXT_GIT_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A git entry displayed by the git picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPickerItem {
    /// File path to open.
    pub path: PathBuf,
    /// Picker root used to render a shorter display label.
    pub root: PathBuf,
    status: GitStatus,
    hunk: Option<DiffHunk>,
}

/// A git picker action target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPickerAction {
    /// File path affected by the action.
    pub path: PathBuf,
    /// Whether the path is untracked.
    pub untracked: bool,
    /// Whether the file currently has staged changes.
    pub staged: bool,
}

/// Picker source for browsing changed git files and hunks.
#[derive(Debug, Clone)]
pub struct GitPickerSource {
    root: PathBuf,
    current_generation: Arc<AtomicU64>,
    preview_generation: Arc<AtomicU64>,
    fuzzy_mode: Arc<AtomicBool>,
    jobs: Arc<JobManager>,
}

/// Search mode used by the git picker.
pub type QueryMode = crate::ui::picker::query::PickerQueryMode;

/// Query passed to the git picker search job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryStyle {
    /// Exact substring search.
    Exact(String),
    /// Fuzzy subsequence search.
    Fuzzy(String),
}

/// Concrete git picker widget.
pub type GitPickerWidget = PickerWidget<GitPickerSource>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GitStatus {
    index: char,
    worktree: char,
}

impl GitStatus {
    fn badge(self) -> String {
        let index = if self.index == ' ' { '.' } else { self.index };
        let worktree = if self.worktree == ' ' {
            '.'
        } else {
            self.worktree
        };
        format!("{index}{worktree}")
    }

    fn display_style(self, base_style: Style) -> Style {
        let theme_name = match (self.index, self.worktree) {
            ('?', '?') | (_, 'A') | ('A', _) => "ui.window.gutter.diff.added",
            (_, 'D') | ('D', _) => "ui.window.gutter.diff.deleted",
            _ => "ui.window.gutter.diff.modified",
        };

        base_style.accent(theme_style(theme_name))
    }

    fn search_text(self) -> String {
        self.badge()
    }

    fn is_untracked(self) -> bool {
        self.index == '?' && self.worktree == '?'
    }

    fn has_staged_changes(self) -> bool {
        self.index != ' '
    }
}

impl GitPickerItem {
    fn new_file(root: PathBuf, path: PathBuf, status: GitStatus) -> Self {
        Self {
            path,
            root,
            status,
            hunk: None,
        }
    }

    fn new_hunk(root: PathBuf, path: PathBuf, status: GitStatus, hunk: DiffHunk) -> Self {
        Self {
            path,
            root,
            status,
            hunk: Some(hunk),
        }
    }

    fn search_text(&self) -> String {
        let mut text = format!(
            "{} {}",
            self.status.search_text(),
            display_path_relative_to(self.root.as_path(), self.path.as_path())
        );
        if let Some(hunk) = self.hunk {
            text.push(' ');
            text.push_str(&hunk_suffix(hunk));
        }
        text
    }

    fn preview_highlight_line(&self) -> Option<usize> {
        self.hunk.map(|hunk| hunk.start_line.saturating_add(1))
    }

    fn action(&self) -> GitPickerAction {
        GitPickerAction {
            path: self.path.clone(),
            untracked: self.status.is_untracked(),
            staged: self.status.has_staged_changes(),
        }
    }
}

impl GitPickerSource {
    /// Creates a git picker rooted at the given directory.
    pub fn new(root: PathBuf) -> Self {
        Self::with_jobs(root, Arc::new(JobManager::new()))
    }

    /// Creates a git picker rooted at the given directory and backed by a shared job manager.
    pub fn with_jobs(root: PathBuf, jobs: Arc<JobManager>) -> Self {
        Self {
            root,
            current_generation: Arc::new(AtomicU64::new(
                NEXT_GIT_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            preview_generation: Arc::new(AtomicU64::new(0)),
            fuzzy_mode: Arc::new(AtomicBool::new(true)),
            jobs,
        }
    }

    /// Returns the current search mode.
    pub fn query_mode(&self) -> QueryMode {
        if self.fuzzy_mode.load(Ordering::SeqCst) {
            QueryMode::Fuzzy
        } else {
            QueryMode::Exact
        }
    }

    /// Sets the current search mode.
    pub fn set_query_mode(&self, mode: QueryMode) {
        self.fuzzy_mode
            .store(matches!(mode, QueryMode::Fuzzy), Ordering::SeqCst);
    }

    /// Toggles the current search mode.
    pub fn toggle_query_mode(&self) -> QueryMode {
        let next = self.query_mode().toggled();
        self.set_query_mode(next);
        next
    }

    /// Returns prompt segments for the current search mode.
    pub fn query_prompt_segments(mode: QueryMode) -> Vec<PromptSegment> {
        query_prompt_segments(mode)
    }
}

impl PickerSource for GitPickerSource {
    type Item = GitPickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        Some(GitPickerSource::toggle_query_mode(self))
    }

    fn query_prompt_segments_for_mode(&self, mode: PickerQueryMode) -> Option<Vec<PromptSegment>> {
        Some(Self::query_prompt_segments(mode))
    }

    fn start_search(
        &self,
        query: &str,
        generation: u64,
        _sender: Sender<PickerSearchEvent<Self::Item>>,
    ) {
        let current_generation = self.current_generation.load(Ordering::SeqCst);
        debug_assert_eq!(current_generation, generation);

        let previous_generation = current_generation.saturating_sub(1);
        if previous_generation > 0 {
            self.jobs.abort_generation(
                crate::background::JobKind::GitPickerSearch,
                JobToken::new(previous_generation),
            );
        }

        let root = self.root.clone();
        let query = match self.query_mode() {
            QueryMode::Exact => QueryStyle::Exact(query.to_string()),
            QueryMode::Fuzzy => QueryStyle::Fuzzy(query.to_string()),
        };
        let token = JobToken::new(generation);
        self.jobs
            .submit(
                crate::background::JobKind::GitPickerSearch,
                token,
                GitPickerSearchJob {
                    root,
                    query,
                    chunk_size: PICKER_CHUNK_SIZE,
                },
            )
            .ok();
    }

    fn preview_key(&self, item: &Self::Item) -> Option<String> {
        Some(item.path.to_string_lossy().into_owned())
    }

    fn result_key(&self, item: &Self::Item) -> Option<String> {
        let mut key = format!("{}|{}", item.path.to_string_lossy(), item.status.badge());
        if let Some(hunk) = item.hunk {
            key.push_str(&format!("|{}:{}", hunk.start_line, hunk.end_line));
        }
        Some(key)
    }

    fn stage_intent(&self, item: &Self::Item) -> Option<Intent> {
        Some(Intent::Command(Command::GitPickerToggleStage(
            item.action(),
        )))
    }

    fn discard_intent(&self, item: &Self::Item) -> Option<Intent> {
        Some(Intent::Command(Command::GitPickerDiscard(item.action())))
    }

    fn start_preview(&self, item: Self::Item, generation: u64, sender: Sender<PickerPreviewEvent>) {
        spawn_preview_loader(
            item,
            generation,
            self.preview_generation.clone(),
            sender,
            build_git_preview,
        );
    }

    fn cancel_preview(&self) {
        self.preview_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn select(&self, item: &Self::Item) -> Intent {
        match item.hunk {
            Some(hunk) => Intent::Command(Command::OpenFileAtCursor(
                item.path.clone(),
                Cursor::new(hunk.start_line, 0),
            )),
            None => Intent::Command(Command::OpenFile(item.path.clone())),
        }
    }

    fn cancel_search(&self) {
        let generation = self.current_generation.load(Ordering::SeqCst);
        if generation == 0 {
            return;
        }

        self.jobs.abort_generation(
            crate::background::JobKind::GitPickerSearch,
            JobToken::new(generation),
        );
    }
}

impl PickerItem for GitPickerItem {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        let label = display_path_relative_to(self.root.as_path(), self.path.as_path());
        let mut sections = Vec::new();
        let mut values: Vec<String> = Vec::new();
        let badge_style = self.status.display_style(base_style).bold();

        push_fixed_text(&mut sections, &mut values, self.status.badge(), badge_style);
        push_fixed_text(&mut sections, &mut values, " ".to_string(), base_style);
        push_file_glyph(&mut sections, &mut values, self.path.as_path(), base_style);
        push_tail_label(&mut sections, &mut values, label, base_style);

        if let Some(hunk) = self.hunk {
            push_fixed_text(
                &mut sections,
                &mut values,
                hunk_suffix(hunk),
                base_style.faint(),
            );
        }

        PickerFormattedLine::new(FormattedLineTemplate::new(sections), values)
    }
}

fn build_git_preview(item: GitPickerItem) -> std::io::Result<PickerPreview> {
    std::fs::metadata(item.path.as_path())?;

    let highlighted_line = item.preview_highlight_line();
    let start_line = highlighted_line
        .map(|line| line.saturating_sub(GIT_PREVIEW_CONTEXT_LINES).max(1))
        .unwrap_or(1);

    Ok(PickerPreview::new(
        item.path.to_string_lossy(),
        start_line,
        highlighted_line,
    ))
}

/// Collects git picker items for a root directory.
pub fn collect_git_picker_items(root: &Path) -> io::Result<Vec<GitPickerItem>> {
    let Some(repo_root) = git_repo_root(root)? else {
        return Ok(Vec::new());
    };

    let mut items = Vec::new();
    for entry in read_status_entries(repo_root.as_path())? {
        let path = repo_root.join(entry.path.as_path());
        let display_root = root.to_path_buf();

        if entry.status.worktree == '?' && entry.status.index == '?' {
            items.push(GitPickerItem::new_file(display_root, path, entry.status));
            continue;
        }

        let hunks = read_diff_hunks(repo_root.as_path(), entry.path.as_path()).unwrap_or_default();
        if hunks.is_empty() {
            items.push(GitPickerItem::new_file(display_root, path, entry.status));
        } else {
            items.extend(hunks.into_iter().map(|hunk| {
                GitPickerItem::new_hunk(display_root.clone(), path.clone(), entry.status, hunk)
            }));
        }
    }

    items.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| {
                left.hunk
                    .map(hunk_sort_key)
                    .cmp(&right.hunk.map(hunk_sort_key))
            })
            .then_with(|| left.status.badge().cmp(&right.status.badge()))
    });

    Ok(items)
}

fn filter_git_picker_items(items: Vec<GitPickerItem>, query: &QueryStyle) -> Vec<GitPickerItem> {
    match query {
        QueryStyle::Exact(query) if query.is_empty() => items,
        QueryStyle::Fuzzy(query) if query.is_empty() => items,
        QueryStyle::Exact(query) => items
            .into_iter()
            .filter(|item| exact_matches(query, item.search_text().as_str()))
            .collect(),
        QueryStyle::Fuzzy(query) => {
            let mut filtered: Vec<(FuzzyMatchScore, GitPickerItem)> = items
                .into_iter()
                .filter_map(|item| {
                    let candidate = item.search_text();
                    fuzzy_match_score(query, candidate.as_str()).map(|score| (score, item))
                })
                .collect();

            filtered.sort_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.path.cmp(&right.1.path))
            });
            filtered.into_iter().map(|(_, item)| item).collect()
        }
    }
}

fn hunk_sort_key(hunk: DiffHunk) -> (usize, usize) {
    (hunk.start_line, hunk.end_line)
}

fn hunk_suffix(hunk: DiffHunk) -> String {
    let start = hunk.start_line.saturating_add(1);
    if hunk.end_line <= hunk.start_line + 1 {
        format!(":{start}")
    } else {
        format!(":{start}-{}", hunk.end_line)
    }
}

fn git_repo_root(root: &Path) -> io::Result<Option<PathBuf>> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
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

#[derive(Debug, Clone)]
struct StatusEntry {
    path: PathBuf,
    status: GitStatus,
}

fn read_status_entries(root: &Path) -> io::Result<Vec<StatusEntry>> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=all")
        .arg("--ignore-submodules=all")
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other("git status failed"));
    }

    Ok(parse_status_entries(&output.stdout))
}

fn parse_status_entries(bytes: &[u8]) -> Vec<StatusEntry> {
    let mut entries = Vec::new();
    let mut offset = 0usize;

    while offset < bytes.len() {
        let Some(record_len) = bytes[offset..].iter().position(|byte| *byte == 0) else {
            break;
        };
        let record = &bytes[offset..offset + record_len];
        offset += record_len + 1;
        if record.len() < 3 {
            continue;
        }

        let status = GitStatus {
            index: record[0] as char,
            worktree: record[1] as char,
        };

        let path = if matches!(status.index, 'R' | 'C') {
            let Some(renamed_len) = bytes[offset..].iter().position(|byte| *byte == 0) else {
                break;
            };
            let new_path = &bytes[offset..offset + renamed_len];
            offset += renamed_len + 1;
            PathBuf::from(OsString::from_vec(new_path.to_vec()))
        } else {
            PathBuf::from(OsString::from_vec(record[3..].to_vec()))
        };

        entries.push(StatusEntry { path, status });
    }

    entries
}

fn read_diff_hunks(root: &Path, relative_path: &Path) -> io::Result<Vec<DiffHunk>> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("diff")
        .arg("HEAD")
        .arg("--no-ext-diff")
        .arg("--no-color")
        .arg("--unified=0")
        .arg("--")
        .arg(relative_path)
        .output()?;

    if !output.status.success() && output.status.code() != Some(1) {
        return Err(io::Error::other("git diff failed"));
    }

    let hunks = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_unified_diff_hunk)
        .collect::<Vec<_>>();
    Ok(merge_hunks(hunks))
}

fn theme_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

#[derive(Debug)]
pub struct GitPickerSearchJob {
    root: PathBuf,
    query: QueryStyle,
    chunk_size: usize,
}

impl GitPickerSearchJob {
    /// Runs the git picker search job on the worker thread.
    pub fn run(
        self,
        context: &JobContext,
        event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    ) {
        let Self {
            root,
            query,
            chunk_size,
        } = self;

        let mut results = Vec::with_capacity(chunk_size);
        let collected = collect_git_picker_items(root.as_path()).unwrap_or_default();
        results.extend(filter_git_picker_items(collected, &query));

        event_tx
            .send(crate::background::JobEvent::Chunk {
                kind: context.kind().clone(),
                token: context.token(),
                payload: JobPayload::GitSearchSnapshot(results),
            })
            .ok();

        event_tx
            .send(crate::background::JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: None,
            })
            .ok();
    }
}

#[cfg(test)]
mod tests;
