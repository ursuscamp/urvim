//! File picker source and selection behavior.

use crate::globals;
use crate::job::{Job, JobContext, JobDelivery, JobKind, JobPriority, JobToken};
use crate::syntax::FiletypeGlyph;
use crate::terminal::Style;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::{
    PickerItem, PickerRenderSegment, PickerSearchEvent, PickerSource, PickerWidget,
};
use crate::ui::{Command, Intent};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;

const PICKER_CHUNK_SIZE: usize = 32;
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
    fuzzy_mode: Arc<AtomicBool>,
}

/// Search mode used by the file picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Exact substring search.
    Exact,
    /// Fuzzy subsequence search.
    Fuzzy,
}

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
        Self {
            root,
            current_generation: Arc::new(AtomicU64::new(
                NEXT_FILE_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            fuzzy_mode: Arc::new(AtomicBool::new(false)),
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
        let next = matches!(self.query_mode(), QueryMode::Exact)
            .then_some(QueryMode::Fuzzy)
            .unwrap_or(QueryMode::Exact);
        self.set_query_mode(next);
        next
    }

    /// Returns prompt segments for the current search mode.
    pub fn query_prompt_segments(mode: QueryMode) -> Vec<PromptSegment> {
        let label = match mode {
            QueryMode::Exact => "Exact",
            QueryMode::Fuzzy => "Fuzzy",
        };

        vec![
            PromptSegment::new(label, highlight_style(mode_prompt_tag(mode))),
            PromptSegment::new(" > ", highlight_style("ui.input.prompt.separator")),
        ]
    }
}

fn highlight_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(name))
            .unwrap_or_default()
    })
}

fn mode_prompt_tag(mode: QueryMode) -> &'static str {
    match mode {
        QueryMode::Exact => "ui.input.prompt.exact",
        QueryMode::Fuzzy => "ui.input.prompt.fuzzy",
    }
}

impl PickerSource for FilePickerSource {
    type Item = FilePickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
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
            globals::with_job_manager(|manager| {
                if let Some(manager) = manager {
                    manager.abort_generation(
                        JobKind::new(FILE_PICKER_SEARCH_JOB_KIND),
                        JobToken::new(previous_generation),
                    );
                }
            });
        }

        if query.is_empty() {
            return;
        }

        let root = self.root.clone();
        let query = match self.query_mode() {
            QueryMode::Exact => QueryStyle::Exact(query.to_string()),
            QueryMode::Fuzzy => QueryStyle::Fuzzy(query.to_string()),
        };
        let token = JobToken::new(generation);
        globals::with_job_manager(|manager| {
            if let Some(manager) = manager {
                let _ = manager.submit(
                    JobKind::new(FILE_PICKER_SEARCH_JOB_KIND),
                    JobPriority::Background,
                    token,
                    JobDelivery::Streaming,
                    PickerSearchJob {
                        root,
                        query,
                        chunk_size: PICKER_CHUNK_SIZE,
                    },
                );
            }
        });
    }

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::OpenFile(item.path.clone()))
    }

    fn cancel_search(&self) {
        let generation = self.current_generation.load(Ordering::SeqCst);
        if generation == 0 {
            return;
        }

        globals::with_job_manager(|manager| {
            if let Some(manager) = manager {
                manager.abort_generation(
                    JobKind::new(FILE_PICKER_SEARCH_JOB_KIND),
                    JobToken::new(generation),
                );
            }
        });
    }
}

impl PickerItem for FilePickerItem {
    fn render_segments(
        &self,
        available_cols: usize,
        base_style: Style,
    ) -> Vec<PickerRenderSegment> {
        let label = display_label(self.root.as_path(), self.path.as_path());
        let mut remaining_cols = available_cols;
        let mut segments = Vec::new();

        if let Some(glyph) = FiletypeGlyph::from_path(self.path.as_path()) {
            let glyph_cols = unicode_width::UnicodeWidthStr::width(glyph.glyph.as_str());
            if remaining_cols > glyph_cols + 1 {
                segments.push(PickerRenderSegment::new(
                    glyph.glyph,
                    base_style.accent(glyph.style),
                ));
                segments.push(PickerRenderSegment::new(" ", base_style));
                remaining_cols = remaining_cols.saturating_sub(glyph_cols + 1);
            }
        }

        let (visible_label, _) =
            crate::ui::picker::visible_tail_text(label.as_str(), remaining_cols, true);
        segments.push(PickerRenderSegment::new(visible_label, base_style));
        segments
    }
}

fn display_label(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn fuzzy_matches(query: &str, candidate: &str) -> bool {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut needle) = query_chars.next() else {
        return true;
    };

    for hay in candidate.chars().flat_map(char::to_lowercase) {
        if hay == needle {
            match query_chars.next() {
                Some(next) => needle = next,
                None => return true,
            }
        }
    }

    false
}

fn exact_matches(query: &str, candidate: &str) -> bool {
    candidate
        .to_lowercase()
        .contains(query.to_lowercase().as_str())
}

const FILE_PICKER_SEARCH_JOB_KIND: &str = "file-picker-search";

#[derive(Debug)]
struct PickerSearchJob {
    root: PathBuf,
    query: QueryStyle,
    chunk_size: usize,
}

impl Job for PickerSearchJob {
    type Output = Vec<FilePickerItem>;

    fn run(self, context: &JobContext, emit: &mut dyn FnMut(Self::Output)) {
        let mut chunk = Vec::with_capacity(self.chunk_size);
        let query = self.query;

        let mut builder = WalkBuilder::new(&self.root);
        builder.standard_filters(true);

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
            let label = display_label(&self.root, &path);
            let matched = match &query {
                QueryStyle::Exact(query) => {
                    exact_matches(query.as_str(), path.to_string_lossy().as_ref())
                        || exact_matches(query.as_str(), label.as_str())
                }
                QueryStyle::Fuzzy(query) => {
                    fuzzy_matches(query.as_str(), path.to_string_lossy().as_ref())
                        || fuzzy_matches(query.as_str(), label.as_str())
                }
            };

            if !matched {
                continue;
            }

            chunk.push(FilePickerItem {
                path,
                root: self.root.clone(),
            });
            if chunk.len() >= self.chunk_size {
                emit(std::mem::take(&mut chunk));
                if context.is_stopping() || context.is_aborted() {
                    return;
                }
            }
        }

        if context.is_stopping() || context.is_aborted() {
            return;
        }

        if !chunk.is_empty() {
            emit(chunk);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::globals;
    use crate::job::{
        JobDelivery, JobEvent, JobHandle, JobKind, JobManager, JobPayload, JobPriority, JobToken,
    };
    use crate::terminal::{Color, Style};
    use crate::theme::{HighlightStyles, Theme, ThemeKind};
    use crate::ui::picker::PickerSearchEvent;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn display_label_strips_root_prefix() {
        let root = PathBuf::from("/tmp/project");
        let path = PathBuf::from("/tmp/project/src/main.rs");
        assert_eq!(display_label(root.as_path(), path.as_path()), "src/main.rs");
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
    fn file_picker_filters_case_insensitively() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("FuzzyMatch.TXT");
        fs::write(&file_path, "hello").unwrap();

        let handle = JobHandle::new();
        handle
            .submit(
                JobKind::new(FILE_PICKER_SEARCH_JOB_KIND),
                JobPriority::Background,
                JobToken::new(1),
                JobDelivery::Streaming,
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
                    payload: JobPayload::FilePickerChunk(chunk),
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
        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_root);

        handle.shutdown();
    }

    #[test]
    fn file_picker_fuzzy_matches_non_substring_queries() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("src").join("file_picker.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "hello").unwrap();

        let handle = JobHandle::new();
        handle
            .submit(
                JobKind::new(FILE_PICKER_SEARCH_JOB_KIND),
                JobPriority::Background,
                JobToken::new(1),
                JobDelivery::Streaming,
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
                    payload: JobPayload::FilePickerChunk(chunk),
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
        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_root);

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
        assert!(fuzzy_matches("fp", "src/ui/file_picker.rs"));
        assert!(fuzzy_matches("FM", "FuzzyMatch.TXT"));
    }

    #[test]
    fn fuzzy_matches_rejects_out_of_order_characters() {
        assert!(!fuzzy_matches("pf", "src/ui/file_picker.rs"));
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
                JobKind::new(FILE_PICKER_SEARCH_JOB_KIND),
                JobPriority::Background,
                JobToken::new(1),
                JobDelivery::Streaming,
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
                    payload: JobPayload::FilePickerChunk(chunk),
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
        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_root);

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
                JobKind::new(FILE_PICKER_SEARCH_JOB_KIND),
                JobPriority::Background,
                JobToken::new(0),
                JobDelivery::Streaming,
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
                    payload: JobPayload::FilePickerChunk(chunk),
                    ..
                } => {
                    picker.handle_search_event(PickerSearchEvent::PickerChunk {
                        generation: token.generation(),
                        chunk,
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

        assert_eq!(picker.highlighted_index(), Some(0));

        manager.shutdown();
        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_root);
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-picker-test-{nanos}"))
    }
}
