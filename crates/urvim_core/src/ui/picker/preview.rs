use crate::background::{JobContext, JobManager, JobToken};
use crate::buffer::{Buffer, BufferCache, BufferCacheRefreshResult, BufferId, Cursor, PieceTable};
use crate::globals;
use crate::screen::Screen;
use crate::window::renderer::{self, BufferRenderState, WindowRenderTheme};
use crate::window::{BufferView, Position, RenderData, Size};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use urvim_terminal::{Color, Style};

use crate::ui::picker::{PickerPreview, PickerPreviewEvent};

/// Starts a background picker preview load with stale-generation protection.
pub fn spawn_preview_loader<T, F>(
    item: T,
    generation: u64,
    current_generation: Arc<AtomicU64>,
    sender: Sender<PickerPreviewEvent>,
    build: F,
) where
    T: Send + 'static,
    F: FnOnce(T) -> std::io::Result<PickerPreview> + Send + 'static,
{
    current_generation.store(generation, Ordering::SeqCst);
    std::thread::spawn(move || {
        sender.send(PickerPreviewEvent::Started { generation }).ok();
        let result = build(item);
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

/// Temporary preview pane backed by an owned buffer outside the global pool.
#[derive(Debug)]
pub struct PreviewPane {
    buffer_view: BufferView,
    render_data: RenderData,
    wrap_enabled: bool,
    follow_highlight: bool,
    syntax_refresh_pending: bool,
    last_viewport_rows: u16,
    jobs: Arc<JobManager>,
}

impl PreviewPane {
    /// Creates a preview pane from an owned buffer.
    pub fn new(buffer: Buffer) -> Self {
        Self::with_jobs(buffer, Arc::new(JobManager::new()))
    }

    /// Creates a preview pane from an owned buffer and shared job manager.
    pub fn with_jobs(buffer: Buffer, jobs: Arc<JobManager>) -> Self {
        Self {
            buffer_view: BufferView::from_owned_buffer(buffer),
            render_data: RenderData::new(0),
            wrap_enabled: false,
            follow_highlight: true,
            syntax_refresh_pending: false,
            last_viewport_rows: 0,
            jobs,
        }
    }

    /// Returns the pane's buffer view.
    pub fn buffer_view(&self) -> &BufferView {
        &self.buffer_view
    }

    /// Returns the pane's buffer view mutably.
    pub fn buffer_view_mut(&mut self) -> &mut BufferView {
        &mut self.buffer_view
    }

    /// Returns whether wrapping is enabled for this preview pane.
    pub fn wrap_enabled(&self) -> bool {
        self.wrap_enabled
    }

    /// Enables or disables wrapping for this preview pane.
    pub fn set_wrap_enabled(&mut self, enabled: bool) {
        self.wrap_enabled = enabled;
    }

    /// Toggles wrapping for this preview pane.
    pub fn toggle_wrap(&mut self) {
        self.wrap_enabled = !self.wrap_enabled;
    }

    /// Returns whether the preview auto-follows the highlighted line.
    pub fn follows_highlight(&self) -> bool {
        self.follow_highlight
    }

    /// Enables or disables auto-following of the highlighted line.
    pub fn set_follow_highlight(&mut self, enabled: bool) {
        self.follow_highlight = enabled;
    }

    /// Returns true when syntax highlighting is still being loaded in the background.
    pub fn syntax_refresh_pending(&self) -> bool {
        self.syntax_refresh_pending
    }

    /// Requests a background syntax refresh for the preview buffer, if needed.
    pub fn request_syntax_refresh(&mut self) {
        self.request_syntax_refresh_for_key(String::new());
    }

    /// Requests a background syntax refresh associated with a stable preview key.
    pub fn request_syntax_refresh_for_key(&mut self, key: String) {
        if self.syntax_refresh_pending {
            return;
        }

        let syntax_enabled = globals::with_config(|config| config.syntax).unwrap_or(true);
        if !syntax_enabled {
            return;
        }

        let Some((syntax_name, line_texts, generation)) = self
            .buffer_view
            .with_buffer(|buffer| {
                if buffer.buffer_cache_complete() || buffer.line_count() == 0 {
                    return None;
                }

                let line_texts = buffer.text_snapshot();

                Some((
                    SmolStr::new(buffer.syntax_name()),
                    line_texts,
                    buffer.syntax_generation(),
                ))
            })
            .flatten()
        else {
            return;
        };

        let job = PreviewSyntaxRefreshJob::new(key, syntax_name, generation, line_texts);
        let submitted = self
            .jobs
            .submit_latest_only(
                crate::background::JobKind::PickerPreviewSyntax,
                JobToken::new(generation),
                job,
            )
            .is_ok();

        if submitted {
            self.syntax_refresh_pending = true;
        }
    }

    /// Applies a completed background syntax refresh to the preview buffer.
    pub fn apply_syntax_refresh_result(&mut self, result: BufferCacheRefreshResult) -> bool {
        self.apply_syntax_refresh_result_inner(result, true)
    }

    /// Applies an in-progress background syntax snapshot to the preview buffer.
    pub fn apply_syntax_refresh_chunk(&mut self, result: BufferCacheRefreshResult) -> bool {
        self.apply_syntax_refresh_result_inner(result, false)
    }

    fn apply_syntax_refresh_result_inner(
        &mut self,
        result: BufferCacheRefreshResult,
        clear_pending: bool,
    ) -> bool {
        let applied = self
            .buffer_view
            .with_buffer_mut(|buffer| buffer.apply_buffer_cache_refresh_result(result))
            .unwrap_or(false);
        if applied && clear_pending {
            self.syntax_refresh_pending = false;
        }
        applied
    }

    /// Marks a pending syntax refresh as failed.
    pub fn clear_syntax_refresh_pending(&mut self) {
        self.syntax_refresh_pending = false;
    }

    /// Scrolls the preview up by one rendered page.
    pub fn page_up(&mut self) {
        self.page_by_pages(true);
    }

    /// Scrolls the preview down by one rendered page.
    pub fn page_down(&mut self) {
        self.page_by_pages(false);
    }

    /// Renders the shared buffer body using the same renderer as editor windows.
    pub fn render(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
        cursor_line: usize,
        active_line: bool,
    ) {
        let (
            gutter_style,
            default_style,
            active_gutter_style,
            active_line_style,
            diff_added_gutter_style,
            diff_deleted_gutter_style,
            diff_modified_gutter_style,
        ) = resolve_window_styles();
        let wrap_mode = globals::with_config(|config| config.wrap_mode).unwrap_or_default();

        self.last_viewport_rows = size.rows;

        let line_count = self.buffer_view.line_count();
        let clamped_cursor_line = cursor_line.min(line_count.saturating_sub(1));
        if active_line {
            self.buffer_view
                .set_cursor_synced(Cursor::new(clamped_cursor_line, 0));
        } else {
            self.buffer_view.set_cursor_synced(Cursor::new(0, 0));
        }

        let mut render_state = BufferRenderState {
            cursor: self.buffer_view.cursor(),
            scroll_offset: self.buffer_view.scroll_offset(),
            wrapped_row_offset: self.buffer_view.wrapped_row_offset(),
            size,
            wrap_enabled: self.wrap_enabled,
            wrap_mode,
            relative_number: false,
            scroll_to_cursor: active_line && self.follow_highlight,
            active_line_enabled: active_line,
            is_normal_mode: active_line,
            syntax_warmup: true,
        };

        renderer::render_buffer_view(
            screen,
            origin,
            &mut self.buffer_view,
            &mut self.render_data,
            WindowRenderTheme {
                gutter_style,
                default_style,
                active_gutter_style: if active_line {
                    Some(active_gutter_style)
                } else {
                    None
                },
                active_line_style: if active_line {
                    Some(active_line_style)
                } else {
                    None
                },
                diff_added_gutter_style,
                diff_deleted_gutter_style,
                diff_modified_gutter_style,
            },
            &mut render_state,
        );
    }

    fn page_by_pages(&mut self, upwards: bool) {
        let viewport_rows = self.last_viewport_rows as usize;
        if viewport_rows == 0 {
            return;
        }

        self.follow_highlight = false;

        let line_count = self.buffer_view.line_count();
        if line_count == 0 {
            self.buffer_view.set_scroll_offset(Position::new(0, 0));
            return;
        }

        let current_row = self.buffer_view.scroll_offset().row as usize;
        let max_top_row = line_count.saturating_sub(viewport_rows);
        let next_row = if upwards {
            current_row.saturating_sub(viewport_rows)
        } else {
            current_row.saturating_add(viewport_rows).min(max_top_row)
        };
        let row = u16::try_from(next_row).unwrap_or(u16::MAX);
        self.buffer_view
            .set_scroll_offset(Position::new(row, self.buffer_view.scroll_offset().col));
    }
}

/// Owns temporary preview panes for a picker session.
#[derive(Debug, Default)]
pub struct PickerPreviewAdapter {
    panes: HashMap<String, PreviewPane>,
    jobs: Arc<JobManager>,
}

impl PickerPreviewAdapter {
    /// Creates an empty preview adapter.
    pub fn new() -> Self {
        Self::with_jobs(Arc::new(JobManager::new()))
    }

    /// Creates an empty preview adapter with a shared job manager.
    pub fn with_jobs(jobs: Arc<JobManager>) -> Self {
        Self {
            panes: HashMap::new(),
            jobs,
        }
    }

    /// Returns the cached preview pane for a file path, loading it on demand.
    pub fn preview_for_path(&mut self, path: &Path) -> std::io::Result<&mut PreviewPane> {
        let key = Self::path_key(path);
        if !self.panes.contains_key(&key) {
            let buffer = Self::load_buffer(path)?;
            self.panes.insert(
                key.clone(),
                PreviewPane::with_jobs(buffer, Arc::clone(&self.jobs)),
            );
        }

        Ok(self.panes.get_mut(&key).expect("cached preview pane"))
    }

    /// Requests syntax refresh for a preview path, loading the preview if needed.
    pub fn request_syntax_refresh_for_path(&mut self, path: &Path) -> std::io::Result<()> {
        let key = Self::path_key(path);
        let pane = self.preview_for_path(path)?;
        pane.request_syntax_refresh_for_key(key);
        Ok(())
    }

    /// Applies a completed syntax refresh to the cached preview for a key.
    pub fn apply_syntax_refresh_result_for_key(
        &mut self,
        key: &str,
        result: BufferCacheRefreshResult,
    ) -> bool {
        self.panes
            .get_mut(key)
            .is_some_and(|pane| pane.apply_syntax_refresh_result(result))
    }

    /// Applies an in-progress syntax snapshot to the cached preview for a key.
    pub fn apply_syntax_refresh_chunk_for_key(
        &mut self,
        key: &str,
        result: BufferCacheRefreshResult,
    ) -> bool {
        self.panes
            .get_mut(key)
            .is_some_and(|pane| pane.apply_syntax_refresh_chunk(result))
    }

    /// Clears the pending syntax-refresh flag for a cached preview.
    pub fn clear_syntax_refresh_pending_for_key(&mut self, key: &str) -> bool {
        self.panes
            .get_mut(key)
            .map(|pane| {
                pane.clear_syntax_refresh_pending();
                true
            })
            .unwrap_or(false)
    }

    /// Inserts or replaces a temporary preview pane.
    pub fn insert(&mut self, key: impl Into<String>, pane: PreviewPane) {
        self.panes.insert(key.into(), pane);
    }

    /// Removes all cached preview panes.
    pub fn clear(&mut self) {
        self.panes.clear();
    }

    /// Returns true when the adapter has a preview pane for the key.
    pub fn contains(&self, key: &str) -> bool {
        self.panes.contains_key(key)
    }

    /// Returns a mutable preview pane for the key, if cached.
    pub fn preview_pane_mut(&mut self, key: &str) -> Option<&mut PreviewPane> {
        self.panes.get_mut(key)
    }

    fn path_key(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    fn load_buffer(path: &Path) -> std::io::Result<Buffer> {
        let contents = fs::read_to_string(path)?;
        if let Some(abs_path) = crate::path::AbsolutePath::from_path(path) {
            Ok(Buffer::from_str_with_path(contents.as_str(), abs_path))
        } else {
            Ok(Buffer::from_str(contents.as_str()))
        }
    }
}

/// Completed syntax refresh data for a specific picker preview pane.
#[derive(Debug, Clone)]
pub struct PreviewSyntaxRefreshResult {
    /// Stable preview key, normally the file path, that requested the refresh.
    pub key: String,
    /// Refreshed syntax cache for that preview buffer snapshot.
    pub result: BufferCacheRefreshResult,
}

#[derive(Debug)]
pub struct PreviewSyntaxRefreshJob {
    key: String,
    syntax_name: SmolStr,
    generation: u64,
    line_texts: PieceTable,
}

impl PreviewSyntaxRefreshJob {
    fn new(key: String, syntax_name: SmolStr, generation: u64, line_texts: PieceTable) -> Self {
        Self {
            key,
            syntax_name,
            generation,
            line_texts,
        }
    }
}

impl PreviewSyntaxRefreshJob {
    /// Runs the preview syntax refresh job on the worker thread.
    pub fn run(
        self,
        context: &JobContext,
        event_tx: &std::sync::mpsc::Sender<crate::background::JobEvent>,
    ) {
        if context.is_stopping() || context.is_aborted() {
            return;
        }

        let mut cache = BufferCache::new(self.syntax_name.clone());
        if !self.line_texts.is_empty() {
            let last_line = self.line_texts.line_count() - 1;
            let mut next_chunk_start = 0usize;
            let chunk_size = 100usize;

            loop {
                if context.is_stopping() || context.is_aborted() {
                    return;
                }

                let chunk_end = next_chunk_start
                    .saturating_add(chunk_size)
                    .saturating_sub(1)
                    .min(last_line);
                cache.ensure_through(&self.syntax_name, &self.line_texts, chunk_end);
                next_chunk_start = chunk_end.saturating_add(1);

                let payload =
                    crate::background::JobPayload::PreviewSyntax(PreviewSyntaxRefreshResult {
                        key: self.key.clone(),
                        result: BufferCacheRefreshResult {
                            buffer_id: BufferId::new(0),
                            generation: self.generation,
                            cache: cache.clone(),
                        },
                    });

                if chunk_end < last_line {
                    event_tx
                        .send(crate::background::JobEvent::Chunk {
                            kind: context.kind().clone(),
                            token: context.token(),
                            payload,
                        })
                        .ok();
                } else {
                    event_tx
                        .send(crate::background::JobEvent::Completed {
                            kind: context.kind().clone(),
                            token: context.token(),
                            payload: Some(payload),
                        })
                        .ok();
                    return;
                }
            }
        }

        event_tx
            .send(crate::background::JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: Some(crate::background::JobPayload::PreviewSyntax(
                    PreviewSyntaxRefreshResult {
                        key: self.key,
                        result: BufferCacheRefreshResult {
                            buffer_id: BufferId::new(0),
                            generation: self.generation,
                            cache,
                        },
                    },
                )),
            })
            .ok();
    }
}

fn resolve_window_styles() -> (Style, Style, Style, Style, Style, Style, Style) {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| {
                (
                    theme.resolve_name_with_default("ui.window.gutter"),
                    theme.default_style(),
                    theme.highlight_style_for_name("ui.window.gutter.active_line"),
                    theme.resolve_name_with_default("ui.window.active_line"),
                    theme.resolve_name_with_default("ui.window.gutter.diff.added"),
                    theme.resolve_name_with_default("ui.window.gutter.diff.deleted"),
                    theme.resolve_name_with_default("ui.window.gutter.diff.modified"),
                )
            })
            .unwrap_or_else(|| {
                (
                    Style::new().bg(Color::ansi(236)).fg(Color::ansi(245)),
                    Style::default(),
                    Style::default(),
                    Style::default(),
                    Style::new().fg(Color::ansi(114)),
                    Style::new().fg(Color::ansi(203)),
                    Style::new().fg(Color::ansi(214)),
                )
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{JobEvent, JobHandle, JobToken};
    use crate::buffer::Buffer;
    use crate::config::{Config, WrapMode};
    use crate::globals;
    use crate::path::AbsolutePath;
    use crate::window::Window;
    use smol_str::SmolStr;
    use std::time::{SystemTime, UNIX_EPOCH};
    use urvim_terminal::{Color, Style};
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    #[test]
    fn preview_pane_pages_by_last_rendered_viewport_height() {
        let mut pane = PreviewPane::new(Buffer::from_str("one\ntwo\nthree\nfour\nfive\n"));
        pane.render(
            &mut crate::screen::Screen::new(2, 20),
            Position::new(0, 0),
            Size::new(2, 20),
            0,
            false,
        );

        pane.page_down();
        assert_eq!(pane.buffer_view().scroll_offset().row, 2);
        assert!(!pane.follows_highlight());

        pane.page_up();
        assert_eq!(pane.buffer_view().scroll_offset().row, 0);
    }

    #[test]
    fn preview_pane_stops_following_highlight_after_manual_scroll() {
        let mut pane = PreviewPane::new(Buffer::from_str("one\ntwo\nthree\nfour\nfive\n"));
        pane.render(
            &mut crate::screen::Screen::new(2, 20),
            Position::new(0, 0),
            Size::new(2, 20),
            4,
            true,
        );
        assert_eq!(pane.buffer_view().scroll_offset().row, 3);

        pane.page_up();
        assert_eq!(pane.buffer_view().scroll_offset().row, 1);

        pane.render(
            &mut crate::screen::Screen::new(2, 20),
            Position::new(0, 0),
            Size::new(2, 20),
            4,
            true,
        );

        assert_eq!(pane.buffer_view().scroll_offset().row, 1);
        assert!(!pane.follows_highlight());
    }

    #[test]
    fn preview_adapter_reuses_buffer_for_path() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "one\ntwo\nthree\n").unwrap();

        let mut adapter = PickerPreviewAdapter::new();
        let first =
            adapter.preview_for_path(file_path.as_path()).unwrap() as *const PreviewPane as usize;
        let second =
            adapter.preview_for_path(file_path.as_path()).unwrap() as *const PreviewPane as usize;

        assert_eq!(first, second);
        assert_eq!(adapter.contains(file_path.to_string_lossy().as_ref()), true);
        assert_eq!(
            adapter
                .panes
                .get(file_path.to_string_lossy().as_ref())
                .unwrap()
                .buffer_view()
                .line_count(),
            3
        );

        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn preview_adapter_loads_full_file_contents_without_touching_pool() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "one\ntwo\nthree\nfour\n").unwrap();

        let before = buffer_count_for_path(file_path.as_path());
        let mut adapter = PickerPreviewAdapter::new();
        let pane = adapter.preview_for_path(file_path.as_path()).unwrap();

        assert_eq!(
            pane.buffer_view()
                .with_buffer(|buffer| buffer.as_str())
                .unwrap(),
            "one\ntwo\nthree\nfour"
        );
        assert!(
            pane.buffer_view()
                .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(3))
                .unwrap()
                .is_none()
        );

        let after = buffer_count_for_path(file_path.as_path());
        assert_eq!(before, after);

        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn preview_pane_renders_plain_text_before_syntax_refresh_completes() {
        let _config_guard = globals::set_test_config(Config {
            syntax: true,
            ..Config::default()
        });
        let theme = themed_window();
        let _theme_guard = globals::set_test_active_theme(theme);
        let file_path = unique_temp_dir().join("preview.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "fn main() { let value = 1; }\n").unwrap();

        let mut pane = PreviewPane::new(Buffer::from_str_with_path(
            "fn main() { let value = 1; }\n",
            AbsolutePath::from_path(file_path.as_path()).unwrap(),
        ));
        assert!(
            pane.buffer_view()
                .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(0))
                .unwrap()
                .is_none()
        );

        let mut preview_screen = crate::screen::Screen::new(2, 32);
        pane.render(
            &mut preview_screen,
            Position::new(0, 0),
            Size::new(2, 32),
            0,
            false,
        );

        let mut plain_window = Window::new(Buffer::from_str_with_path(
            "fn main() { let value = 1; }\n",
            AbsolutePath::from_path(file_path.as_path()).unwrap(),
        ));
        plain_window.set_cursor(crate::buffer::Cursor::new(0, 0));
        let mut plain_screen = crate::screen::Screen::new(2, 32);
        plain_window.render(&mut plain_screen, Position::new(0, 0), Size::new(2, 32));

        assert_screen_body_eq(&mut preview_screen, &mut plain_screen);

        fs::remove_file(file_path).ok();
    }

    #[test]
    fn preview_adapter_requests_and_applies_async_syntax_refresh() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "fn main() { let value = 1; }\n").unwrap();

        let mut adapter = PickerPreviewAdapter::new();
        adapter
            .request_syntax_refresh_for_path(file_path.as_path())
            .unwrap();
        let key = file_path.to_string_lossy().into_owned();
        let generation = adapter
            .preview_pane_mut(&key)
            .unwrap()
            .buffer_view()
            .with_buffer(|buffer| buffer.syntax_generation())
            .unwrap();
        assert!(
            adapter
                .preview_pane_mut(&key)
                .unwrap()
                .syntax_refresh_pending()
        );

        let handle = JobHandle::new();
        handle
            .submit_latest_only(
                crate::background::JobKind::PickerPreviewSyntax,
                JobToken::new(generation),
                PreviewSyntaxRefreshJob::new(
                    key.clone(),
                    SmolStr::new("rust"),
                    generation,
                    PieceTable::from_text("fn main() { let value = 1; }"),
                ),
            )
            .unwrap();

        let result = match wait_for_event(&handle) {
            JobEvent::Completed {
                payload: Some(crate::background::JobPayload::PreviewSyntax(preview_result)),
                ..
            } => {
                assert_eq!(preview_result.key, key);
                preview_result.result
            }
            other => panic!("expected preview syntax completion, got {:?}", other),
        };

        assert!(adapter.apply_syntax_refresh_result_for_key(&key, result));
        assert!(
            !adapter
                .preview_pane_mut(&key)
                .unwrap()
                .syntax_refresh_pending()
        );

        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn preview_adapter_ignores_stale_syntax_refresh_results() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "fn main() { let value = 1; }\n").unwrap();

        let mut adapter = PickerPreviewAdapter::new();
        adapter
            .request_syntax_refresh_for_path(file_path.as_path())
            .unwrap();
        let key = file_path.to_string_lossy().into_owned();
        assert!(
            adapter
                .preview_pane_mut(&key)
                .unwrap()
                .syntax_refresh_pending()
        );

        let stale_result = BufferCacheRefreshResult {
            buffer_id: BufferId::new(0),
            generation: 999,
            cache: BufferCache::new("rust"),
        };

        assert!(!adapter.apply_syntax_refresh_result_for_key(&key, stale_result));
        assert!(
            adapter
                .preview_pane_mut(&key)
                .unwrap()
                .syntax_refresh_pending()
        );

        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn preview_pane_marks_syntax_refresh_complete_after_failure_notification() {
        let mut pane = PreviewPane::new(Buffer::from_str("one\ntwo\n"));
        pane.request_syntax_refresh();
        assert!(pane.syntax_refresh_pending());

        pane.clear_syntax_refresh_pending();

        assert!(!pane.syntax_refresh_pending());
    }

    #[test]
    fn preview_syntax_refresh_job_populates_cached_spans_off_thread() {
        let line_texts = PieceTable::from_text(
            std::iter::repeat_n("fn main() { let value = 1; }", 250)
                .collect::<Vec<_>>()
                .join("\n")
                .as_str(),
        );
        let handle = JobHandle::new();
        handle
            .submit_latest_only(
                crate::background::JobKind::TestPickerPreviewSyntax,
                JobToken::new(7),
                PreviewSyntaxRefreshJob::new(
                    String::from("/tmp/preview.rs"),
                    SmolStr::new("rust"),
                    7,
                    line_texts,
                ),
            )
            .unwrap();

        let mut chunk_count = 0;
        let result = loop {
            let event = wait_for_event(&handle);
            match event {
                JobEvent::Chunk {
                    payload: crate::background::JobPayload::PreviewSyntax(preview_result),
                    ..
                } => {
                    chunk_count += 1;
                    assert_eq!(preview_result.key, "/tmp/preview.rs");
                }
                JobEvent::Completed {
                    payload: Some(crate::background::JobPayload::PreviewSyntax(preview_result)),
                    ..
                } => {
                    assert_eq!(preview_result.key, "/tmp/preview.rs");
                    break preview_result.result;
                }
                other => panic!(
                    "expected preview syntax chunk or completion, got {:?}",
                    other
                ),
            }
        };

        assert!(chunk_count > 0);
        assert_eq!(result.generation, 7);
        assert!(result.cache.cached_spans_for_line(0).is_some());
        assert!(result.cache.is_complete_for_line_count(250));

        handle.shutdown();
    }

    #[test]
    fn preview_pane_renders_body_like_window() {
        let _config_guard = globals::set_test_config(Config {
            wrap_mode: WrapMode::Hard,
            syntax: false,
            ..Config::default()
        });
        let theme = themed_window();
        let _theme_guard = globals::set_test_active_theme(theme);

        let mut pane = PreviewPane::new(Buffer::from_str("alpha\nbeta\n"));
        let mut preview_screen = crate::screen::Screen::new(2, 20);
        pane.render(
            &mut preview_screen,
            Position::new(0, 0),
            Size::new(2, 20),
            0,
            false,
        );

        let mut window = Window::new(Buffer::from_str("alpha\nbeta\n"));
        window.set_cursor(crate::buffer::Cursor::new(0, 0));
        let mut window_screen = crate::screen::Screen::new(2, 20);
        window.render(&mut window_screen, Position::new(0, 0), Size::new(2, 20));

        assert_eq!(
            row_text(&mut preview_screen, 0, 0),
            row_text(&mut window_screen, 0, 0)
        );
        assert_eq!(
            row_text(&mut preview_screen, 1, 0),
            row_text(&mut window_screen, 1, 0)
        );
    }

    #[test]
    fn preview_pane_matches_window_for_syntax_and_active_line() {
        let _config_guard = globals::set_test_config(Config {
            wrap_mode: WrapMode::Hard,
            syntax: false,
            active_line: true,
            ..Config::default()
        });
        let theme = themed_window();
        let _theme_guard = globals::set_test_active_theme(theme);
        let file_path = unique_temp_dir().join("preview.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "fn main() { let value = 1; }\n").unwrap();

        let mut pane = PreviewPane::new(Buffer::from_str_with_path(
            "fn main() { let value = 1; }\n",
            AbsolutePath::from_path(file_path.as_path()).unwrap(),
        ));
        let mut preview_screen = crate::screen::Screen::new(2, 32);
        pane.render(
            &mut preview_screen,
            Position::new(0, 0),
            Size::new(2, 32),
            0,
            true,
        );

        let mut window = Window::new(Buffer::from_str_with_path(
            "fn main() { let value = 1; }\n",
            AbsolutePath::from_path(file_path.as_path()).unwrap(),
        ));
        window.set_cursor(crate::buffer::Cursor::new(0, 0));
        let mut window_screen = crate::screen::Screen::new(2, 32);
        window.render(&mut window_screen, Position::new(0, 0), Size::new(2, 32));

        assert_screen_body_eq(&mut preview_screen, &mut window_screen);

        fs::remove_file(file_path).ok();
    }

    #[test]
    fn preview_pane_matches_window_when_wrapping() {
        let _config_guard = globals::set_test_config(Config {
            wrap_mode: WrapMode::Hard,
            syntax: false,
            ..Config::default()
        });
        let theme = themed_window();
        let _theme_guard = globals::set_test_active_theme(theme);
        let mut pane = PreviewPane::new(Buffer::from_str("abcdefghij\nklmnop\n"));
        pane.set_wrap_enabled(true);
        let mut preview_screen = crate::screen::Screen::new(2, 8);
        pane.render(
            &mut preview_screen,
            Position::new(0, 0),
            Size::new(2, 8),
            0,
            false,
        );

        let mut window = Window::new(Buffer::from_str("abcdefghij\nklmnop\n"));
        window.set_wrap_enabled(true);
        window.set_cursor(crate::buffer::Cursor::new(0, 0));
        let mut window_screen = crate::screen::Screen::new(2, 8);
        window.render(&mut window_screen, Position::new(0, 0), Size::new(2, 8));

        assert_eq!(
            row_text(&mut preview_screen, 0, 0),
            row_text(&mut window_screen, 0, 0)
        );
    }

    #[test]
    fn preview_pane_reuses_cache_for_same_path_across_match_changes() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        fs::write(&file_path, "one\ntwo\nthree\nfour\n").unwrap();

        let mut adapter = PickerPreviewAdapter::new();
        let first =
            adapter.preview_for_path(file_path.as_path()).unwrap() as *const PreviewPane as usize;
        adapter
            .preview_for_path(file_path.as_path())
            .unwrap()
            .render(
                &mut crate::screen::Screen::new(2, 20),
                Position::new(0, 0),
                Size::new(2, 20),
                0,
                false,
            );
        let second =
            adapter.preview_for_path(file_path.as_path()).unwrap() as *const PreviewPane as usize;
        adapter
            .preview_for_path(file_path.as_path())
            .unwrap()
            .render(
                &mut crate::screen::Screen::new(2, 20),
                Position::new(0, 0),
                Size::new(2, 20),
                1,
                true,
            );

        assert_eq!(first, second);

        fs::remove_file(file_path).ok();
        fs::remove_dir_all(temp_root).ok();
    }

    fn buffer_count_for_path(path: &Path) -> usize {
        crate::globals::with_buffer_pool(|pool| {
            pool.buffer_ids()
                .into_iter()
                .filter(|id| {
                    pool.get(*id)
                        .and_then(|buffer| buffer.path())
                        .is_some_and(|buffer_path| buffer_path.as_path() == path)
                })
                .count()
        })
    }

    fn wait_for_event(handle: &JobHandle) -> JobEvent {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(event) = handle.poll_event() {
                return event;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for job event"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn row_text(screen: &mut crate::screen::Screen, row: u16, start_col: u16) -> String {
        let (_, cols) = screen.size();
        (start_col..cols)
            .map(|col| screen.get_cell_mut(row, col).unwrap().text.clone())
            .collect()
    }

    fn assert_screen_body_eq(left: &mut crate::screen::Screen, right: &mut crate::screen::Screen) {
        let (rows, _) = left.size();
        assert_eq!(left.size(), right.size());
        for row in 0..rows {
            let left_text = row_text(left, row, 0);
            let right_text = row_text(right, row, 0);
            let left_body = left_text.trim_start().trim_end();
            let right_body = right_text.trim_start().trim_end();
            assert!(
                left_body.starts_with(right_body) || right_body.starts_with(left_body),
                "body mismatch at row {row}: {left_body:?} vs {right_body:?}"
            );
        }
    }

    fn themed_window() -> Theme {
        let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.selection").expect("valid tag"),
            Style::new().reverse(),
        );
        highlights.insert(
            Tag::parse("ui.window.active_line").expect("valid tag"),
            Style::new().bg(Color::ansi(21)),
        );
        highlights.insert(
            Tag::parse("ui.window.gutter").expect("valid tag"),
            Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
        );
        highlights.insert(
            Tag::parse("ui.window").expect("valid tag"),
            Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines").expect("valid tag"),
            Style::new().fg(Color::ansi(15)).bg(Color::ansi(16)),
        );
        highlights.insert(
            Tag::parse("syntax.comment").expect("valid tag"),
            Style::new().fg(Color::ansi(20)),
        );
        highlights.insert(
            Tag::parse("syntax.constant").expect("valid tag"),
            Style::new().fg(Color::ansi(21)),
        );
        highlights.insert(
            Tag::parse("syntax.function").expect("valid tag"),
            Style::new().fg(Color::ansi(22)),
        );
        highlights.insert(
            Tag::parse("syntax.keyword").expect("valid tag"),
            Style::new().fg(Color::ansi(23)),
        );
        highlights.insert(
            Tag::parse("syntax.operator").expect("valid tag"),
            Style::new().fg(Color::ansi(24)),
        );
        highlights.insert(
            Tag::parse("syntax.punctuation").expect("valid tag"),
            Style::new().fg(Color::ansi(25)),
        );
        highlights.insert(
            Tag::parse("syntax.string").expect("valid tag"),
            Style::new().fg(Color::ansi(26)),
        );
        highlights.insert(
            Tag::parse("syntax.type").expect("valid tag"),
            Style::new().fg(Color::ansi(27)),
        );
        highlights.insert(
            Tag::parse("syntax.variable").expect("valid tag"),
            Style::new().fg(Color::ansi(28)),
        );
        for tag_name in [
            "syntax.comment.todo",
            "syntax.comment.fixme",
            "syntax.comment.bug",
            "syntax.comment.note",
        ] {
            highlights.insert(Tag::parse(tag_name).expect("valid tag"), Style::new());
        }
        Theme::new("demo", ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn preview_pane_matches_window_with_syntax_and_active_line() {
        let _config_guard = globals::set_test_config(Config {
            wrap_mode: WrapMode::Hard,
            syntax: true,
            active_line: true,
            ..Config::default()
        });
        let theme = themed_window();
        let _theme_guard = globals::set_test_active_theme(theme);
        let file_path = unique_temp_dir().join("preview.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(
            &file_path,
            "fn main() { let value: Option<String> = Some(\"hi\"); } // note\n",
        )
        .unwrap();

        let mut pane = PreviewPane::new(Buffer::from_str_with_path(
            "fn main() { let value: Option<String> = Some(\"hi\"); } // note\n",
            AbsolutePath::from_path(file_path.as_path()).unwrap(),
        ));
        let mut preview_screen = crate::screen::Screen::new(1, 80);
        pane.render(
            &mut preview_screen,
            Position::new(0, 0),
            Size::new(1, 80),
            0,
            true,
        );

        let mut window = Window::new(Buffer::from_str_with_path(
            "fn main() { let value: Option<String> = Some(\"hi\"); } // note\n",
            AbsolutePath::from_path(file_path.as_path()).unwrap(),
        ));
        window.set_cursor(crate::buffer::Cursor::new(0, 0));
        let mut window_screen = crate::screen::Screen::new(1, 80);
        window.render(&mut window_screen, Position::new(0, 0), Size::new(1, 80));

        assert_screen_body_eq(&mut preview_screen, &mut window_screen);

        fs::remove_file(file_path).ok();
    }

    #[test]
    fn preview_pane_warms_initial_viewport_syntax_cache_on_render() {
        let _config_guard = globals::set_test_config(Config {
            syntax: true,
            ..Config::default()
        });
        let theme = themed_window();
        let _theme_guard = globals::set_test_active_theme(theme);

        let mut pane = PreviewPane::new(Buffer::from_str(
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n",
        ));
        pane.render(
            &mut crate::screen::Screen::new(2, 20),
            Position::new(0, 0),
            Size::new(2, 20),
            0,
            false,
        );

        assert!(
            pane.buffer_view()
                .with_buffer(|buffer| buffer.cached_syntax_spans_for_line(0))
                .unwrap()
                .is_some()
        );
    }

    fn unique_temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-preview-adapter-{nanos}"))
    }
}
