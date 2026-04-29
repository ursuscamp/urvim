//! Live grep picker source and selection behavior.

use crate::buffer::Cursor;
use crate::globals;
use crate::job::{Job, JobContext, JobDelivery, JobKind, JobPriority, JobToken};
use crate::syntax::FiletypeGlyph;
use crate::terminal::Style;
use crate::ui::picker::{
    PickerItem, PickerRenderSegment, PickerSearchEvent, PickerSource, PickerWidget,
};
use crate::ui::{Command, Intent};
use ignore::WalkBuilder;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

const PICKER_CHUNK_SIZE: usize = 32;
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
        }
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
        let query = query.to_string();
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

        if remaining_cols <= suffix_cols + 1 {
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

        let path_budget = remaining_cols.saturating_sub(suffix_cols + 1);
        let (visible_label, _) =
            crate::ui::picker::visible_tail_text(label.as_str(), path_budget, true);
        segments.push(PickerRenderSegment::new(visible_label, base_style));
        segments.push(PickerRenderSegment::new(" ", base_style));
        segments.push(PickerRenderSegment::new(suffix, suffix_style));
        segments
    }
}

fn display_label(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
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
    query: String,
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

                if let Some(column) = line.find(query.as_str()) {
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
                    query: "target".to_string(),
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
                    query: "needle".to_string(),
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
