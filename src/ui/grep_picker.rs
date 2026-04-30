//! Live grep picker source and selection behavior.

use crate::buffer::Cursor;
use crate::globals;
use crate::job::{Job, JobContext, JobDelivery, JobKind, JobPriority, JobToken};
use crate::syntax::FiletypeGlyph;
use crate::terminal::Style;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::{
    PickerItem, PickerPreview, PickerPreviewEvent, PickerRenderSegment, PickerSearchEvent,
    PickerSource, PickerWidget, picker_indicator_glyph,
};
use crate::ui::{Command, Intent};
use ignore::WalkBuilder;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

const PICKER_CHUNK_SIZE: usize = 32;
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
}

/// Search mode used by the live grep picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Exact substring search.
    Exact,
    /// Fuzzy subsequence search.
    Fuzzy,
}

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
        Self {
            root,
            current_generation: Arc::new(AtomicU64::new(
                NEXT_GREP_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            preview_generation: Arc::new(AtomicU64::new(0)),
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
            PromptSegment::new(
                format!(" {} ", picker_indicator_glyph()),
                highlight_style("ui.input.prompt.separator"),
            ),
        ]
    }
}

impl PickerSource for GrepPickerSource {
    type Item = GrepPickerItem;

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
                        JobKind::new(GREP_PICKER_SEARCH_JOB_KIND),
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
                    JobKind::new(GREP_PICKER_SEARCH_JOB_KIND),
                    JobPriority::Background,
                    token,
                    JobDelivery::Streaming,
                    GrepPickerSearchJob {
                        root,
                        query,
                        chunk_size: PICKER_CHUNK_SIZE,
                    },
                );
            }
        });
    }

    fn preview_key(&self, item: &Self::Item) -> Option<String> {
        Some(item.path.to_string_lossy().into_owned())
    }

    fn start_preview(&self, item: Self::Item, generation: u64, sender: Sender<PickerPreviewEvent>) {
        self.preview_generation.store(generation, Ordering::SeqCst);
        let current_generation = self.preview_generation.clone();
        thread::spawn(move || {
            sender.send(PickerPreviewEvent::Started { generation }).ok();
            let result = build_grep_preview(&item);
            if current_generation.load(Ordering::SeqCst) != generation {
                return;
            }

            match result {
                Ok(preview) => sender
                    .send(PickerPreviewEvent::Loaded {
                        generation,
                        preview,
                    })
                    .ok(),
                Err(error) => sender
                    .send(PickerPreviewEvent::Failed {
                        generation,
                        message: error.to_string(),
                    })
                    .ok(),
            };
        });
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

        globals::with_job_manager(|manager| {
            if let Some(manager) = manager {
                manager.abort_generation(
                    JobKind::new(GREP_PICKER_SEARCH_JOB_KIND),
                    JobToken::new(generation),
                );
            }
        });
    }
}

impl PickerItem for GrepPickerItem {
    fn render_segments(
        &self,
        available_cols: usize,
        base_style: Style,
    ) -> Vec<PickerRenderSegment> {
        let label = display_label(self.root.as_path(), self.path.as_path());
        let suffix = format!(":{}:{}", self.line + 1, self.column + 1);
        let mut remaining_cols = available_cols;
        let mut segments = Vec::new();
        let suffix_style = base_style.faint().accent(location_style());
        let suffix_cols = unicode_width::UnicodeWidthStr::width(suffix.as_str());

        if remaining_cols <= suffix_cols {
            let (visible_suffix, _) =
                crate::ui::picker::visible_tail_text(suffix.as_str(), remaining_cols, true);
            return vec![PickerRenderSegment::new(visible_suffix, suffix_style)];
        }

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

        let path_budget = remaining_cols.saturating_sub(suffix_cols);
        let (visible_label, _) =
            crate::ui::picker::visible_tail_text(label.as_str(), path_budget, true);
        segments.push(PickerRenderSegment::new(visible_label, base_style));
        segments.push(PickerRenderSegment::new(suffix, suffix_style));
        segments
    }
}

fn build_grep_preview(item: &GrepPickerItem) -> std::io::Result<PickerPreview> {
    let start_line = item.line.saturating_sub(GREP_PREVIEW_CONTEXT_LINES);
    let _ = std::fs::metadata(item.path.as_path())?;

    Ok(PickerPreview::new(
        item.path.to_string_lossy(),
        start_line + 1,
        Some(item.line + 1),
    ))
}

fn display_label(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
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

fn exact_matches(query: &str, candidate: &str) -> bool {
    candidate
        .to_lowercase()
        .contains(query.to_lowercase().as_str())
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

fn fuzzy_match_column(query: &str, candidate: &str) -> usize {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut needle) = query_chars.next() else {
        return 0;
    };

    let mut first_match = None;
    for (byte_idx, hay) in candidate
        .char_indices()
        .flat_map(|(idx, ch)| ch.to_lowercase().map(move |lower| (idx, lower)))
    {
        if hay == needle {
            first_match.get_or_insert(byte_idx);
            match query_chars.next() {
                Some(next) => needle = next,
                None => return first_match.unwrap_or(0),
            }
        }
    }

    first_match.unwrap_or(0)
}

fn location_style() -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default("ui.picker.location"))
            .unwrap_or_default()
    })
}

pub const GREP_PICKER_SEARCH_JOB_KIND: &str = "grep-picker-search";

#[derive(Debug)]
struct GrepPickerSearchJob {
    root: PathBuf,
    query: QueryStyle,
    chunk_size: usize,
}

impl Job for GrepPickerSearchJob {
    type Output = Vec<GrepPickerItem>;

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

                let matched_column = match &query {
                    QueryStyle::Exact(query) => {
                        let lower_line = line.to_lowercase();
                        let lower_query = query.to_lowercase();
                        exact_matches(query.as_str(), line.as_str())
                            .then(|| lower_line.find(lower_query.as_str()).unwrap_or(0))
                    }
                    QueryStyle::Fuzzy(query) => fuzzy_matches(query.as_str(), line.as_str())
                        .then(|| fuzzy_match_column(query.as_str(), line.as_str())),
                };

                if let Some(column) = matched_column {
                    chunk.push(GrepPickerItem {
                        path: path.clone(),
                        root: self.root.clone(),
                        line: line_index,
                        column,
                    });

                    if chunk.len() >= self.chunk_size {
                        emit(std::mem::take(&mut chunk));
                        if context.is_stopping() || context.is_aborted() {
                            return;
                        }
                    }
                }

                line_index += 1;
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
    use crate::job::{
        JobDelivery, JobEvent, JobHandle, JobKind, JobManager, JobPayload, JobPriority, JobToken,
    };
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
                JobKind::new(GREP_PICKER_SEARCH_JOB_KIND),
                JobPriority::Background,
                JobToken::new(1),
                JobDelivery::Streaming,
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
                    payload: JobPayload::GrepPickerChunk(chunk),
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

        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_root);
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
        assert!(fuzzy_matches("fp", "src/ui/file_picker.rs"));
        assert!(fuzzy_matches("GM", "gamma target"));
    }

    #[test]
    fn grep_picker_fuzzy_matches_rejects_out_of_order_characters() {
        assert!(!fuzzy_matches("pf", "src/ui/file_picker.rs"));
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
                JobKind::new(GREP_PICKER_SEARCH_JOB_KIND),
                JobPriority::Background,
                token,
                JobDelivery::Streaming,
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
                    payload: JobPayload::GrepPickerChunk(chunk),
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

        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_root);
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
