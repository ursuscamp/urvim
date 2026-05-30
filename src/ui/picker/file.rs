//! File picker source and selection behavior.

use crate::background::JobPayload;
use crate::background::{JobContext, JobManager, JobToken};
use crate::terminal::Style;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::line::{display_path_relative_to, push_file_glyph, push_tail_label};
use crate::ui::picker::preview::spawn_preview_loader;
use crate::ui::picker::query::{
    FuzzyMatchScore, PickerQueryMode, exact_matches, fuzzy_match_score, query_prompt_segments,
};
use crate::ui::picker::{
    FormattedLineTemplate, PickerFormattedLine, PickerItem, PickerPreview, PickerPreviewEvent,
    PickerSearchEvent, PickerSource, PickerWidget,
};
use crate::ui::{Command, Intent};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

#[cfg(test)]
use crate::ui::picker::query::fuzzy_matches;

const PICKER_CHUNK_SIZE: usize = 32;
const RANKED_FLUSH_INTERVAL: Duration = Duration::from_millis(50);
static NEXT_FILE_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A file entry displayed by the file picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickerItem {
    /// File path to open.
    pub path: PathBuf,
    root: PathBuf,
}

/// Picker source for searching files beneath a root directory.
#[derive(Debug, Clone)]
pub struct FilePickerSource {
    root: PathBuf,
    current_generation: Arc<AtomicU64>,
    preview_generation: Arc<AtomicU64>,
    fuzzy_mode: Arc<AtomicBool>,
    jobs: Arc<JobManager>,
}

/// Search mode used by the file picker.
pub type QueryMode = crate::ui::picker::query::PickerQueryMode;

/// Query passed to the file picker search job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryStyle {
    /// Exact substring search.
    Exact(String),
    /// Fuzzy subsequence search.
    Fuzzy(String),
}

/// Concrete file picker widget.
pub type FilePickerWidget = PickerWidget<FilePickerSource>;

impl FilePickerSource {
    /// Creates a file picker rooted at the given directory.
    pub fn new(root: PathBuf) -> Self {
        Self::with_jobs(root, Arc::new(JobManager::new()))
    }

    /// Creates a file picker rooted at the given directory and backed by a shared job manager.
    pub fn with_jobs(root: PathBuf, jobs: Arc<JobManager>) -> Self {
        Self {
            root,
            current_generation: Arc::new(AtomicU64::new(
                NEXT_FILE_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
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

impl PickerSource for FilePickerSource {
    type Item = FilePickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        Some(FilePickerSource::toggle_query_mode(self))
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
                crate::background::JobKind::FilePickerSearch,
                JobToken::new(previous_generation),
            );
        }

        if query.is_empty() {
            _sender
                .send(PickerSearchEvent::PickerSearchComplete { generation })
                .ok();
            return;
        }

        let root = self.root.clone();
        let query = match self.query_mode() {
            QueryMode::Exact => QueryStyle::Exact(query.to_string()),
            QueryMode::Fuzzy => QueryStyle::Fuzzy(query.to_string()),
        };
        let token = JobToken::new(generation);
        self.jobs
            .submit(
                crate::background::JobKind::FilePickerSearch,
                token,
                PickerSearchJob {
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
        Some(item.path.to_string_lossy().into_owned())
    }

    fn start_preview(&self, item: Self::Item, generation: u64, sender: Sender<PickerPreviewEvent>) {
        spawn_preview_loader(
            item,
            generation,
            self.preview_generation.clone(),
            sender,
            |item| build_file_preview(item.path.as_path()),
        );
    }

    fn cancel_preview(&self) {
        self.preview_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::OpenFile(item.path.clone()))
    }

    fn cancel_search(&self) {
        let generation = self.current_generation.load(Ordering::SeqCst);
        if generation == 0 {
            return;
        }

        self.jobs.abort_generation(
            crate::background::JobKind::FilePickerSearch,
            JobToken::new(generation),
        );
    }
}

impl PickerItem for FilePickerItem {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        let label = display_path_relative_to(self.root.as_path(), self.path.as_path());
        let mut sections = Vec::new();
        let mut values: Vec<String> = Vec::new();

        push_file_glyph(&mut sections, &mut values, self.path.as_path(), base_style);
        push_tail_label(&mut sections, &mut values, label, base_style);

        PickerFormattedLine::new(FormattedLineTemplate::new(sections), values)
    }
}

fn build_file_preview(path: &Path) -> std::io::Result<PickerPreview> {
    std::fs::metadata(path)?;

    Ok(PickerPreview::new(path.to_string_lossy(), 1, None))
}

#[derive(Debug)]
pub struct PickerSearchJob {
    root: PathBuf,
    query: QueryStyle,
    chunk_size: usize,
}

#[derive(Debug, Clone)]
struct RankedFilePickerItem {
    score: FuzzyMatchScore,
    item: FilePickerItem,
}

impl PickerSearchJob {
    /// Runs the file picker search job on the worker thread.
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

        let mut builder = WalkBuilder::new(&root);
        builder.standard_filters(true);

        match query {
            QueryStyle::Exact(query) => {
                let mut results = Vec::with_capacity(chunk_size);
                let mut last_flush = Instant::now();

                for entry in builder.build().filter_map(Result::ok) {
                    if context.is_stopping() || context.is_aborted() {
                        return;
                    }

                    if !entry
                        .file_type()
                        .is_some_and(|file_type| file_type.is_file())
                    {
                        continue;
                    }

                    let path = entry.path().to_path_buf();
                    let label = display_path_relative_to(&root, &path);
                    let matched = exact_matches(query.as_str(), path.to_string_lossy().as_ref())
                        || exact_matches(query.as_str(), label.as_str());

                    if !matched {
                        continue;
                    }

                    results.push(FilePickerItem {
                        path,
                        root: root.clone(),
                    });

                    if last_flush.elapsed() >= RANKED_FLUSH_INTERVAL {
                        flush_file_snapshot(event_tx, context, &results);
                        last_flush = Instant::now();
                    }
                }

                if context.is_stopping() || context.is_aborted() {
                    return;
                }

                flush_file_snapshot(event_tx, context, &results);
            }

            QueryStyle::Fuzzy(query) => {
                let mut ranked = Vec::new();
                let mut last_flush = Instant::now();
                let mut last_sent_count = 0usize;

                for entry in builder.build().filter_map(Result::ok) {
                    if context.is_stopping() || context.is_aborted() {
                        return;
                    }

                    if !entry
                        .file_type()
                        .is_some_and(|file_type| file_type.is_file())
                    {
                        continue;
                    }

                    let path = entry.path().to_path_buf();
                    let label = display_path_relative_to(&root, &path);
                    let Some(score) =
                        file_fuzzy_score(query.as_str(), path.as_path(), label.as_str())
                    else {
                        continue;
                    };

                    ranked.push(RankedFilePickerItem {
                        score,
                        item: FilePickerItem {
                            path,
                            root: root.clone(),
                        },
                    });

                    if last_flush.elapsed() >= RANKED_FLUSH_INTERVAL {
                        flush_ranked_file_snapshot(
                            event_tx,
                            context,
                            &mut ranked,
                            &mut last_sent_count,
                        );
                        last_flush = Instant::now();
                    }
                }

                if context.is_stopping() || context.is_aborted() {
                    return;
                }

                flush_ranked_file_snapshot(event_tx, context, &mut ranked, &mut last_sent_count);
            }
        }

        event_tx
            .send(crate::background::JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: None,
            })
            .ok();
    }
}

fn file_fuzzy_score(query: &str, path: &Path, label: &str) -> Option<FuzzyMatchScore> {
    let path_score = fuzzy_match_score(query, path.to_string_lossy().as_ref());
    let label_score = fuzzy_match_score(query, label);
    match (path_score, label_score) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(score), None) | (None, Some(score)) => Some(score),
        (None, None) => None,
    }
}

fn flush_ranked_file_snapshot(
    event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    context: &JobContext,
    ranked: &mut Vec<RankedFilePickerItem>,
    last_sent_count: &mut usize,
) {
    if ranked.len() == *last_sent_count {
        return;
    }

    ranked.sort_by(|left, right| left.score.cmp(&right.score));
    let results = ranked.iter().map(|entry| entry.item.clone()).collect();
    event_tx
        .send(crate::background::JobEvent::Chunk {
            kind: context.kind().clone(),
            token: context.token(),
            payload: JobPayload::FileSearchSnapshot(results),
        })
        .ok();
    *last_sent_count = ranked.len();
}

fn flush_file_snapshot(
    event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    context: &JobContext,
    results: &[FilePickerItem],
) {
    event_tx
        .send(crate::background::JobEvent::Chunk {
            kind: context.kind().clone(),
            token: context.token(),
            payload: JobPayload::FileSearchSnapshot(results.to_vec()),
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{JobEvent, JobHandle, JobManager, JobToken};
    use crate::globals;
    use crate::terminal::{Color, Style};
    use crate::theme::{HighlightStyles, Theme, ThemeKind};
    use crate::ui::picker::PickerSearchEvent;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn display_label_strips_root_prefix() {
        let root = PathBuf::from("/tmp/project");
        let path = PathBuf::from("/tmp/project/src/main.rs");
        assert_eq!(
            display_path_relative_to(root.as_path(), path.as_path()),
            "src/main.rs"
        );
    }

    #[test]
    fn file_picker_selects_open_file_intent() {
        let source = FilePickerSource::new(PathBuf::from("/tmp"));
        let intent = source.select(&FilePickerItem {
            path: PathBuf::from("/tmp/example.txt"),
            root: PathBuf::from("/tmp"),
        });

        assert!(matches!(intent, Intent::Command(Command::OpenFile(_))));
    }

    #[test]
    fn file_picker_item_renders_tail_label_without_icon_when_nerdfont_disabled() {
        let item = FilePickerItem {
            path: PathBuf::from("/tmp/project/src/deep/main.rs"),
            root: PathBuf::from("/tmp/project"),
        };

        let segments = item.render_segments(12, Style::default());

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "…eep/main.rs");
    }

    #[test]
    fn file_picker_preview_reads_start_of_file() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "fn main() {}\nlet tail = true;\n").unwrap();

        let preview = build_file_preview(file_path.as_path()).expect("preview");

        assert_eq!(preview.start_line, 1);
        assert_eq!(preview.highlighted_line, None);
    }

    #[test]
    fn file_picker_preview_keeps_full_file_contents() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "one\ntwo\nthree\nfour\n").unwrap();

        let preview = build_file_preview(file_path.as_path()).expect("preview");

        assert_eq!(preview.start_line, 1);
        assert_eq!(preview.highlighted_line, None);
    }

    #[test]
    fn file_picker_filters_case_insensitively() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("FuzzyMatch.TXT");
        fs::write(&file_path, "hello").unwrap();

        let handle = JobHandle::new();
        handle
            .submit(
                crate::background::JobKind::FilePickerSearch,
                JobToken::new(1),
                PickerSearchJob {
                    root: temp_root.clone(),
                    query: QueryStyle::Exact("fuzzy".to_string()),
                    chunk_size: PICKER_CHUNK_SIZE,
                },
            )
            .unwrap();

        let mut saw_match = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !saw_match {
            match handle.poll_event() {
                Some(JobEvent::Chunk {
                    payload: JobPayload::FileSearchSnapshot(chunk),
                    ..
                }) => {
                    saw_match = chunk.iter().any(|item| item.path == file_path);
                }
                Some(_) => {}
                None => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "timed out waiting for streamed picker results"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }

        assert!(saw_match);
        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();

        handle.shutdown();
    }

    #[test]
    fn file_picker_fuzzy_matches_non_substring_queries() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("src").join("picker").join("file.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "hello").unwrap();

        let handle = JobHandle::new();
        handle
            .submit(
                crate::background::JobKind::FilePickerSearch,
                JobToken::new(1),
                PickerSearchJob {
                    root: temp_root.clone(),
                    query: QueryStyle::Fuzzy("fp".to_string()),
                    chunk_size: PICKER_CHUNK_SIZE,
                },
            )
            .unwrap();

        let mut saw_match = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !saw_match {
            match handle.poll_event() {
                Some(JobEvent::Chunk {
                    payload: JobPayload::FileSearchSnapshot(chunk),
                    ..
                }) => {
                    saw_match = chunk.iter().any(|item| item.path == file_path);
                }
                Some(_) => {}
                None => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "timed out waiting for streamed picker results"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }

        assert!(saw_match);
        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();

        handle.shutdown();
    }

    #[test]
    fn file_picker_source_toggles_query_mode() {
        let source = FilePickerSource::new(PathBuf::from("/tmp"));

        assert_eq!(source.query_mode(), QueryMode::Exact);
        assert_eq!(source.toggle_query_mode(), QueryMode::Fuzzy);
        assert_eq!(source.query_mode(), QueryMode::Fuzzy);
        source.set_query_mode(QueryMode::Exact);
        assert_eq!(source.query_mode(), QueryMode::Exact);
    }

    #[test]
    fn query_prompt_segments_include_mode_label() {
        let theme = Theme::new(
            "prompt-test",
            ThemeKind::Ansi256,
            Style::default(),
            HighlightStyles::new(
                [
                    (
                        "ui.input.prompt.exact",
                        Style::new().fg(Color::ansi(1)).bold(),
                    ),
                    (
                        "ui.input.prompt.fuzzy",
                        Style::new().fg(Color::ansi(2)).italic(),
                    ),
                    (
                        "ui.input.prompt.separator",
                        Style::new().fg(Color::ansi(3)).faint(),
                    ),
                ]
                .into_iter()
                .map(|(name, style)| (crate::theme::Tag::parse(name).expect("valid tag"), style))
                .collect(),
            ),
        );
        let _theme_guard = globals::set_test_active_theme(theme);

        let exact = FilePickerSource::query_prompt_segments(QueryMode::Exact);
        let fuzzy = FilePickerSource::query_prompt_segments(QueryMode::Fuzzy);

        assert_eq!(exact.len(), 2);
        assert_eq!(exact[0].text, "Exact");
        assert_eq!(exact[1].text, " > ");
        assert_eq!(fuzzy[0].text, "Fuzzy");
        assert_eq!(fuzzy[1].text, " > ");
        assert_eq!(exact[0].style, Style::new().fg(Color::ansi(1)).bold());
        assert_eq!(fuzzy[0].style, Style::new().fg(Color::ansi(2)).italic());
        assert_eq!(exact[1].style, Style::new().fg(Color::ansi(3)).faint());
        assert_eq!(fuzzy[1].style, Style::new().fg(Color::ansi(3)).faint());
    }

    #[test]
    fn fuzzy_matches_subsequence_case_insensitively() {
        assert!(fuzzy_matches("fp", "src/ui/file/path.rs"));
        assert!(fuzzy_matches("FM", "FuzzyMatch.TXT"));
    }

    #[test]
    fn fuzzy_matches_rejects_out_of_order_characters() {
        assert!(!fuzzy_matches("pf", "src/ui/file/path.rs"));
    }

    #[test]
    fn fuzzy_match_score_prefers_tighter_file_matches() {
        let tight = file_fuzzy_score(
            "fp",
            Path::new("/tmp/src/ui/file/path.rs"),
            "src/ui/file/path.rs",
        )
        .expect("tight score");
        let loose = file_fuzzy_score(
            "fp",
            Path::new("/tmp/src/ui/features/path.rs"),
            "src/ui/features/path.rs",
        )
        .expect("loose score");

        assert!(tight < loose);
    }

    #[test]
    fn file_picker_exact_matches_case_insensitively() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("FuzzyMatch.TXT");
        fs::write(&file_path, "hello").unwrap();

        let handle = JobHandle::new();
        handle
            .submit(
                crate::background::JobKind::FilePickerSearch,
                JobToken::new(1),
                PickerSearchJob {
                    root: temp_root.clone(),
                    query: QueryStyle::Exact("fuzzy".to_string()),
                    chunk_size: PICKER_CHUNK_SIZE,
                },
            )
            .unwrap();

        let mut saw_match = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !saw_match {
            match handle.poll_event() {
                Some(JobEvent::Chunk {
                    payload: JobPayload::FileSearchSnapshot(chunk),
                    ..
                }) => {
                    saw_match = chunk.iter().any(|item| item.path == file_path);
                }
                Some(_) => {}
                None => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "timed out waiting for streamed picker results"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }

        assert!(saw_match);
        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();

        handle.shutdown();
    }

    #[test]
    fn file_picker_streams_results_through_the_job_manager() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("FuzzyMatch.TXT");
        fs::write(&file_path, "hello").unwrap();

        let manager = JobManager::new();
        manager
            .submit(
                crate::background::JobKind::FilePickerSearch,
                JobToken::new(0),
                PickerSearchJob {
                    root: temp_root.clone(),
                    query: QueryStyle::Exact("fuzzy".to_string()),
                    chunk_size: PICKER_CHUNK_SIZE,
                },
            )
            .unwrap();

        let source = FilePickerSource::new(temp_root.clone());
        let mut picker = PickerWidget::new(source);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !picker.results().iter().any(|item| item.path == file_path) {
            let _ = manager.process_events(|event| match event {
                JobEvent::Started { .. } => {}
                JobEvent::Chunk {
                    token,
                    payload: JobPayload::FileSearchSnapshot(chunk),
                    ..
                } => {
                    picker.handle_search_event(PickerSearchEvent::PickerResults {
                        generation: token.generation(),
                        results: chunk,
                    });
                }
                JobEvent::Chunk { .. } => {}
                JobEvent::Completed { token, .. } | JobEvent::Failed { token, .. } => {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            });

            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for streamed picker results"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert_eq!(picker.highlighted_index(), None);

        manager.shutdown();
        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-picker-test-{nanos}"))
    }
}
