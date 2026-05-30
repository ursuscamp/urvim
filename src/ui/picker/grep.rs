//! Live grep picker source and selection behavior.

use crate::background::JobPayload;
use crate::background::{JobContext, JobKind, JobManager, JobToken};
use crate::buffer::Cursor;
use crate::terminal::Style;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::line::{
    display_path_relative_to, push_file_glyph, push_fixed_text, push_tail_label,
};
use crate::ui::picker::preview::spawn_preview_loader;
use crate::ui::picker::query::{
    FuzzyMatchScore, PickerQueryMode, exact_matches, fuzzy_match_column, fuzzy_match_score,
    query_prompt_segments,
};
use crate::ui::picker::{
    FormattedLineTemplate, PickerFormattedLine, PickerItem, PickerPreview, PickerPreviewEvent,
    PickerSearchEvent, PickerSource, PickerWidget,
};
use crate::ui::{Command, Intent};
use ignore::WalkBuilder;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

#[cfg(test)]
use crate::ui::picker::query::fuzzy_matches;

const PICKER_CHUNK_SIZE: usize = 32;
const RANKED_FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const GREP_PREVIEW_CONTEXT_LINES: usize = 100;
static NEXT_GREP_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A grep match displayed by the live grep picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepPickerItem {
    /// File path containing the match.
    pub path: PathBuf,
    /// Picker root used to render a shorter display label.
    pub root: PathBuf,
    /// Matched line number, zero-based.
    pub line: usize,
    /// Matched column, zero-based byte offset.
    pub column: usize,
}

/// Picker source for searching line matches beneath a root directory.
#[derive(Debug, Clone)]
pub struct GrepPickerSource {
    root: PathBuf,
    current_generation: Arc<AtomicU64>,
    preview_generation: Arc<AtomicU64>,
    fuzzy_mode: Arc<AtomicBool>,
    jobs: Arc<JobManager>,
}

/// Search mode used by the live grep picker.
pub type QueryMode = crate::ui::picker::query::PickerQueryMode;

/// Query passed to the live grep search job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryStyle {
    /// Exact substring search.
    Exact(String),
    /// Fuzzy subsequence search.
    Fuzzy(String),
}

/// Concrete live grep picker widget.
pub type GrepPickerWidget = PickerWidget<GrepPickerSource>;

impl GrepPickerSource {
    /// Creates a live grep picker rooted at the given directory.
    pub fn new(root: PathBuf) -> Self {
        Self::with_jobs(root, Arc::new(JobManager::new()))
    }

    /// Creates a live grep picker rooted at the given directory and backed by a shared job manager.
    pub fn with_jobs(root: PathBuf, jobs: Arc<JobManager>) -> Self {
        Self {
            root,
            current_generation: Arc::new(AtomicU64::new(
                NEXT_GREP_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            preview_generation: Arc::new(AtomicU64::new(0)),
            fuzzy_mode: Arc::new(AtomicBool::new(false)),
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

impl PickerSource for GrepPickerSource {
    type Item = GrepPickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        Some(GrepPickerSource::toggle_query_mode(self))
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
                JobKind::GrepPickerSearch,
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
                JobKind::GrepPickerSearch,
                token,
                GrepPickerSearchJob {
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
        Some(format!(
            "{}:{}:{}",
            item.path.to_string_lossy(),
            item.line,
            item.column
        ))
    }

    fn start_preview(&self, item: Self::Item, generation: u64, sender: Sender<PickerPreviewEvent>) {
        spawn_preview_loader(
            item,
            generation,
            self.preview_generation.clone(),
            sender,
            |item| build_grep_preview(&item),
        );
    }

    fn cancel_preview(&self) {
        self.preview_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::OpenFileAtCursor(
            item.path.clone(),
            Cursor::new(item.line, item.column),
        ))
    }

    fn cancel_search(&self) {
        let generation = self.current_generation.load(Ordering::SeqCst);
        if generation == 0 {
            return;
        }

        self.jobs
            .abort_generation(JobKind::GrepPickerSearch, JobToken::new(generation));
    }
}

impl PickerItem for GrepPickerItem {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        let label = display_path_relative_to(self.root.as_path(), self.path.as_path());
        let suffix = format!(":{}:{}", self.line + 1, self.column + 1);
        let suffix_style = base_style.faint().accent(location_style());
        let mut sections = Vec::new();
        let mut values: Vec<String> = Vec::new();

        push_file_glyph(&mut sections, &mut values, self.path.as_path(), base_style);
        push_tail_label(&mut sections, &mut values, label, base_style);
        push_fixed_text(&mut sections, &mut values, suffix, suffix_style);

        PickerFormattedLine::new(FormattedLineTemplate::new(sections), values)
    }
}

fn build_grep_preview(item: &GrepPickerItem) -> std::io::Result<PickerPreview> {
    let start_line = item.line.saturating_sub(GREP_PREVIEW_CONTEXT_LINES);
    std::fs::metadata(item.path.as_path())?;

    Ok(PickerPreview::new(
        item.path.to_string_lossy(),
        start_line + 1,
        Some(item.line + 1),
    ))
}

fn location_style() -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default("ui.picker.location"))
            .unwrap_or_default()
    })
}

#[derive(Debug)]
pub struct GrepPickerSearchJob {
    root: PathBuf,
    query: QueryStyle,
    chunk_size: usize,
}

#[derive(Debug, Clone)]
struct RankedGrepPickerItem {
    score: FuzzyMatchScore,
    item: GrepPickerItem,
}

impl GrepPickerSearchJob {
    /// Runs the live grep search job on the worker thread.
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
                    let file = match File::open(&path) {
                        Ok(file) => file,
                        Err(_) => continue,
                    };

                    let mut reader = BufReader::new(file);
                    let mut line = String::new();
                    let mut line_index = 0usize;

                    loop {
                        if context.is_stopping() || context.is_aborted() {
                            return;
                        }

                        line.clear();
                        let read = match reader.read_line(&mut line) {
                            Ok(read) => read,
                            Err(_) => break,
                        };
                        if read == 0 {
                            break;
                        }

                        let matched_column =
                            exact_matches(query.as_str(), line.as_str()).then(|| {
                                let lower_line = line.to_lowercase();
                                let lower_query = query.to_lowercase();
                                lower_line.find(lower_query.as_str()).unwrap_or(0)
                            });

                        if let Some(column) = matched_column {
                            results.push(GrepPickerItem {
                                path: path.clone(),
                                root: root.clone(),
                                line: line_index,
                                column,
                            });

                            if last_flush.elapsed() >= RANKED_FLUSH_INTERVAL {
                                flush_grep_snapshot(event_tx, context, &results);
                                last_flush = Instant::now();
                            }
                        }

                        line_index += 1;
                    }
                }

                if context.is_stopping() || context.is_aborted() {
                    return;
                }

                flush_grep_snapshot(event_tx, context, &results);
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
                    let file = match File::open(&path) {
                        Ok(file) => file,
                        Err(_) => continue,
                    };

                    let mut reader = BufReader::new(file);
                    let mut line = String::new();
                    let mut line_index = 0usize;

                    loop {
                        if context.is_stopping() || context.is_aborted() {
                            return;
                        }

                        line.clear();
                        let read = match reader.read_line(&mut line) {
                            Ok(read) => read,
                            Err(_) => break,
                        };
                        if read == 0 {
                            break;
                        }

                        let Some(score) = fuzzy_match_score(query.as_str(), line.as_str()) else {
                            line_index += 1;
                            continue;
                        };

                        let column = fuzzy_match_column(query.as_str(), line.as_str());
                        ranked.push(RankedGrepPickerItem {
                            score,
                            item: GrepPickerItem {
                                path: path.clone(),
                                root: root.clone(),
                                line: line_index,
                                column,
                            },
                        });

                        if last_flush.elapsed() >= RANKED_FLUSH_INTERVAL {
                            flush_ranked_grep_snapshot(
                                event_tx,
                                context,
                                &mut ranked,
                                &mut last_sent_count,
                            );
                            last_flush = Instant::now();
                        }

                        line_index += 1;
                    }
                }

                if context.is_stopping() || context.is_aborted() {
                    return;
                }

                flush_ranked_grep_snapshot(event_tx, context, &mut ranked, &mut last_sent_count);
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

fn flush_ranked_grep_snapshot(
    event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    context: &JobContext,
    ranked: &mut Vec<RankedGrepPickerItem>,
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
            payload: JobPayload::GrepSearchSnapshot(results),
        })
        .ok();
    *last_sent_count = ranked.len();
}

fn flush_grep_snapshot(
    event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    context: &JobContext,
    results: &[GrepPickerItem],
) {
    event_tx
        .send(crate::background::JobEvent::Chunk {
            kind: context.kind().clone(),
            token: context.token(),
            payload: JobPayload::GrepSearchSnapshot(results.to_vec()),
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{JobEvent, JobHandle, JobKind, JobManager, JobToken};
    use crate::globals;
    use crate::terminal::Style;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn grep_picker_selects_open_file_at_cursor_intent() {
        let source = GrepPickerSource::new(PathBuf::from("/tmp"));
        let intent = source.select(&GrepPickerItem {
            path: PathBuf::from("/tmp/example.txt"),
            root: PathBuf::from("/tmp"),
            line: 7,
            column: 3,
        });

        assert!(matches!(
            intent,
            Intent::Command(Command::OpenFileAtCursor(_, _))
        ));
    }

    #[test]
    fn grep_picker_item_renders_tail_label_and_location_suffix() {
        let item = GrepPickerItem {
            path: PathBuf::from("/tmp/project/src/deep/main.rs"),
            root: PathBuf::from("/tmp/project"),
            line: 9,
            column: 4,
        };

        let segments = item.render_segments(24, Style::default());

        assert!(!segments.is_empty());
        assert!(
            segments
                .iter()
                .any(|segment| segment.text.contains(":10:5"))
        );
        assert!(
            !segments
                .windows(2)
                .any(|pair| pair[0].text.ends_with(' ') && pair[1].text.starts_with(":"))
        );
    }

    #[test]
    fn grep_picker_preview_includes_lines_around_match() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "one\ntwo\nmatch\nfour\nfive\n").unwrap();
        let item = GrepPickerItem {
            path: file_path,
            root: temp_root,
            line: 2,
            column: 0,
        };

        let preview = build_grep_preview(&item).expect("preview");

        assert_eq!(preview.start_line, 1);
        assert_eq!(preview.highlighted_line, Some(3));
    }

    #[test]
    fn grep_picker_preview_key_uses_file_path_only() {
        let source = GrepPickerSource::new(PathBuf::from("/tmp"));
        let first = GrepPickerItem {
            path: PathBuf::from("/tmp/example.txt"),
            root: PathBuf::from("/tmp"),
            line: 1,
            column: 0,
        };
        let second = GrepPickerItem {
            path: PathBuf::from("/tmp/example.txt"),
            root: PathBuf::from("/tmp"),
            line: 99,
            column: 0,
        };

        assert_eq!(source.preview_key(&first), source.preview_key(&second));
    }

    #[test]
    fn grep_picker_finds_matching_lines() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("sample.txt");
        fs::write(&file_path, "alpha\nbeta target\ngamma target\n").unwrap();

        let handle = JobHandle::new();
        handle
            .submit(
                JobKind::GrepPickerSearch,
                JobToken::new(1),
                GrepPickerSearchJob {
                    root: temp_root.clone(),
                    query: QueryStyle::Exact("target".to_string()),
                    chunk_size: 2,
                },
            )
            .unwrap();

        let mut matches = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while matches.len() < 2 {
            match handle.poll_event() {
                Some(JobEvent::Chunk {
                    payload: JobPayload::GrepSearchSnapshot(chunk),
                    ..
                }) => matches.extend(chunk),
                Some(_) => {}
                None => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "timed out waiting for grep results"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].path, file_path);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[0].column, 5);
        assert_eq!(matches[1].line, 2);
        assert_eq!(matches[1].column, 6);

        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
        handle.shutdown();
    }

    #[test]
    fn grep_picker_source_toggles_query_mode() {
        let source = GrepPickerSource::new(PathBuf::from("/tmp"));

        assert_eq!(source.query_mode(), QueryMode::Exact);
        assert_eq!(source.toggle_query_mode(), QueryMode::Fuzzy);
        assert_eq!(source.query_mode(), QueryMode::Fuzzy);
        source.set_query_mode(QueryMode::Exact);
        assert_eq!(source.query_mode(), QueryMode::Exact);
    }

    #[test]
    fn query_prompt_segments_include_mode_label() {
        let theme = crate::theme::Theme::new(
            "prompt-test",
            crate::theme::ThemeKind::Ansi256,
            Style::default(),
            crate::theme::HighlightStyles::new(
                [
                    (
                        "ui.input.prompt.exact",
                        Style::new().fg(crate::terminal::Color::ansi(1)).bold(),
                    ),
                    (
                        "ui.input.prompt.fuzzy",
                        Style::new().fg(crate::terminal::Color::ansi(2)).italic(),
                    ),
                    (
                        "ui.input.prompt.separator",
                        Style::new().fg(crate::terminal::Color::ansi(3)).faint(),
                    ),
                ]
                .into_iter()
                .map(|(name, style)| (crate::theme::Tag::parse(name).expect("valid tag"), style))
                .collect(),
            ),
        );
        let _theme_guard = globals::set_test_active_theme(theme);

        let exact = GrepPickerSource::query_prompt_segments(QueryMode::Exact);
        let fuzzy = GrepPickerSource::query_prompt_segments(QueryMode::Fuzzy);

        assert_eq!(exact.len(), 2);
        assert_eq!(exact[0].text, "Exact");
        assert_eq!(exact[1].text, " > ");
        assert_eq!(fuzzy[0].text, "Fuzzy");
        assert_eq!(fuzzy[1].text, " > ");
        assert_eq!(
            exact[0].style,
            Style::new().fg(crate::terminal::Color::ansi(1)).bold()
        );
        assert_eq!(
            fuzzy[0].style,
            Style::new().fg(crate::terminal::Color::ansi(2)).italic()
        );
        assert_eq!(
            exact[1].style,
            Style::new().fg(crate::terminal::Color::ansi(3)).faint()
        );
        assert_eq!(
            fuzzy[1].style,
            Style::new().fg(crate::terminal::Color::ansi(3)).faint()
        );
    }

    #[test]
    fn grep_picker_fuzzy_matches_subsequence_case_insensitively() {
        assert!(fuzzy_matches("fp", "src/ui/file/path.rs"));
        assert!(fuzzy_matches("GM", "gamma target"));
    }

    #[test]
    fn grep_picker_fuzzy_matches_rejects_out_of_order_characters() {
        assert!(!fuzzy_matches("pf", "src/ui/file/path.rs"));
    }

    #[test]
    fn fuzzy_match_score_prefers_tighter_grep_matches() {
        let tight = fuzzy_match_score("gt", "gamma target").expect("tight score");
        let loose = fuzzy_match_score("gt", "g a m m a   t a r g e t").expect("loose score");

        assert!(tight < loose);
    }

    #[test]
    fn grep_picker_streams_results_through_the_job_manager() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("stream.txt");
        fs::write(&file_path, "needle\nother\nneedle\n").unwrap();

        let manager = JobManager::new();
        let token = JobToken::new(1);
        manager
            .submit(
                JobKind::GrepPickerSearch,
                token,
                GrepPickerSearchJob {
                    root: temp_root.clone(),
                    query: QueryStyle::Exact("needle".to_string()),
                    chunk_size: 1,
                },
            )
            .unwrap();

        let mut saw_match = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !saw_match {
            manager.process_events(|event| match event {
                JobEvent::Chunk {
                    payload: JobPayload::GrepSearchSnapshot(chunk),
                    ..
                } => {
                    saw_match = chunk.iter().any(|item| item.path == file_path);
                }
                _ => {}
            });
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for streamed grep results"
            );
            if !saw_match {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
        manager.shutdown();
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-grep-picker-test-{nanos}"))
    }
}
