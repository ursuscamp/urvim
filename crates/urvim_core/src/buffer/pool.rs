//! Global buffer pool and buffer identifiers.
//!
//! The buffer pool owns all live buffers in the editor and assigns each one a
//! stable `BufferId`. It also deduplicates file-backed buffers by absolute
//! path so opening the same file twice reuses the existing in-memory buffer.
//! Mutable access runs through the pool while the pool is locked so edits stay
//! synchronized across threads.

use super::Buffer;
use super::diff::DiffRefreshJob;
use super::syntax::{IndentScopeRefreshJob, SyntaxRefreshJob};
use crate::background::JobPayload;
use crate::background::{BackgroundJob, JobEvent, JobKind, JobManager, JobSubmitError, JobToken};
use crate::event::{
    BufferErrorSnapshot, BufferEventSnapshot, BufferPathChangeSnapshot, EditorEvent,
};
use crate::globals;
use crate::path::AbsolutePath;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// Stable identifier for a buffer stored in the global buffer pool.
pub use urvim_id::BufferId;

#[derive(Debug)]
pub struct BufferPool {
    next_id: usize,
    buffers: HashMap<BufferId, Buffer>,
    paths: HashMap<AbsolutePath, BufferId>,
    jobs: JobManager,
    last_disk_check: Option<Instant>,
    reported_external_conflicts: HashMap<BufferId, Option<super::DiskState>>,
}

impl BufferPool {
    /// Creates an empty buffer pool.
    pub fn new() -> Self {
        Self {
            next_id: 0,
            buffers: HashMap::new(),
            paths: HashMap::new(),
            jobs: JobManager::new(),
            last_disk_check: None,
            reported_external_conflicts: HashMap::new(),
        }
    }

    /// Creates a new empty buffer and returns its identifier.
    pub fn create_buffer(&mut self) -> BufferId {
        self.insert_buffer(Buffer::new())
    }

    /// Registers an existing buffer in the pool, reusing an existing entry if
    /// the buffer is already backed by a known absolute path.
    pub fn register_buffer(&mut self, buffer: Buffer) -> BufferId {
        self.insert_buffer(buffer)
    }

    /// Creates a new empty buffer with a resolved path and returns its
    /// identifier.
    pub fn create_buffer_with_path(&mut self, path: impl AsRef<Path>) -> io::Result<BufferId> {
        let abs_path = AbsolutePath::from_path(path.as_ref()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "failed to resolve absolute path",
            )
        })?;

        if let Some(id) = self.paths.get(&abs_path).copied() {
            return Ok(id);
        }

        Ok(self.insert_buffer(Buffer::with_path(abs_path)))
    }

    /// Opens a file-backed buffer from a path, reusing an existing buffer if the
    /// same absolute path is already present in the pool.
    ///
    /// If the path does not exist yet, this creates an empty buffer that still
    /// remembers the resolved path so a later save will create the file.
    pub fn open_buffer(&mut self, path: impl AsRef<Path>) -> io::Result<BufferId> {
        let abs_path = match AbsolutePath::from_path(path.as_ref()) {
            Some(path) => path,
            None => {
                let error = io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "failed to resolve absolute path",
                );
                globals::enqueue_editor_event(EditorEvent::BufferOpenFailed {
                    error: BufferErrorSnapshot::from_io_error(None, None, &error),
                });
                return Err(error);
            }
        };

        if let Some(id) = self.paths.get(&abs_path).copied() {
            return Ok(id);
        }

        match Buffer::load_from_file(abs_path.as_path()) {
            Ok(mut buffer) => {
                buffer.set_path(abs_path.clone());
                Ok(self.insert_buffer(buffer))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(self.insert_buffer(Buffer::with_path(abs_path)))
            }
            Err(error) => {
                globals::enqueue_editor_event(EditorEvent::BufferOpenFailed {
                    error: BufferErrorSnapshot::from_io_error(
                        None,
                        Some(abs_path.as_path().to_path_buf()),
                        &error,
                    ),
                });
                Err(error)
            }
        }
    }

    /// Returns an immutable reference to a buffer by ID.
    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.get(&id)
    }

    /// Returns a mutable reference to a buffer by ID.
    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.buffers.get_mut(&id)
    }

    /// Returns the buffer identifier currently associated with a file path.
    pub fn buffer_id_for_path(&self, path: &AbsolutePath) -> Option<BufferId> {
        self.paths.get(path).copied()
    }

    /// Updates the stored path for a loaded buffer.
    pub fn rename_buffer_path(&mut self, id: BufferId, path: impl AsRef<Path>) -> io::Result<()> {
        let abs_path = AbsolutePath::from_path(path.as_ref()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "failed to resolve absolute path",
            )
        })?;

        let Some(buffer) = self.buffers.get_mut(&id) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "buffer id not found",
            ));
        };

        if let Some(existing_id) = self.paths.get(&abs_path).copied()
            && existing_id != id
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "buffer path already exists in pool",
            ));
        }

        let old_path = buffer.path().cloned();
        if let Some(old_path) = &old_path {
            self.paths.remove(&old_path);
        }

        buffer.set_path(abs_path.clone());
        self.paths.insert(abs_path, id);
        self.reported_external_conflicts.remove(&id);
        let snapshot = BufferEventSnapshot::from_buffer(id, buffer);
        globals::enqueue_editor_event(EditorEvent::BufferPathChanged {
            snapshot: BufferPathChangeSnapshot {
                buffer: snapshot,
                previous_path: old_path.map(|path| path.as_path().to_path_buf()),
            },
        });
        Ok(())
    }

    /// Removes a buffer from the pool and returns it when present.
    ///
    /// Removing a buffer enqueues a [`EditorEvent::BufferUnloaded`] event with
    /// a snapshot of the buffer's metadata so plugin hooks can observe the
    /// unload after the buffer is no longer live in the pool.
    pub fn remove_buffer(&mut self, id: BufferId) -> Option<Buffer> {
        let buffer = self.buffers.remove(&id)?;
        if let Some(path) = buffer.path().cloned() {
            self.paths.remove(&path);
        }
        self.reported_external_conflicts.remove(&id);
        let snapshot = BufferEventSnapshot::from_buffer(id, &buffer);
        globals::enqueue_editor_event(EditorEvent::BufferUnloaded { snapshot });
        Some(buffer)
    }

    /// Returns all buffer identifiers currently loaded into the pool.
    pub fn buffer_ids(&self) -> Vec<BufferId> {
        let mut ids = self.buffers.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    }

    /// Returns the identifiers for modified buffers currently loaded into the pool.
    pub fn modified_buffer_ids(&self) -> Vec<BufferId> {
        let mut ids = self
            .buffers
            .iter()
            .filter_map(|(id, buffer)| buffer.is_modified().then_some(*id))
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    }

    /// Returns how many modified buffers are currently loaded into the pool.
    pub fn modified_buffer_count(&self) -> usize {
        self.buffers
            .values()
            .filter(|buffer| buffer.is_modified())
            .count()
    }

    /// Requests a background job to be scheduled on the buffer-pool engine.
    pub fn submit_background_job<J>(
        &mut self,
        kind: JobKind,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Into<BackgroundJob>,
    {
        self.jobs.submit_latest_only(kind, token, job)
    }

    /// Processes completed background jobs owned by the buffer pool.
    pub fn process_background_jobs(&mut self) -> bool {
        let mut accepted_redraw = false;

        while let Some(event) = self.jobs.poll_event() {
            match event {
                JobEvent::Chunk {
                    payload: JobPayload::SyntaxRefresh(result),
                    ..
                } => {
                    if self
                        .with_buffer_mut(result.buffer_id, |buffer| {
                            buffer.apply_syntax_refresh_result(result)
                        })
                        .unwrap_or(false)
                    {
                        accepted_redraw = true;
                    }
                }
                JobEvent::Completed {
                    payload: Some(JobPayload::SyntaxRefresh(result)),
                    ..
                } => {
                    if self
                        .with_buffer_mut(result.buffer_id, |buffer| {
                            buffer.apply_syntax_refresh_result(result)
                        })
                        .unwrap_or(false)
                    {
                        accepted_redraw = true;
                    }
                }
                JobEvent::Completed {
                    kind: JobKind::SyntaxRefresh(buffer_id),
                    token,
                    payload: None,
                } => {
                    if self
                        .with_buffer_mut(buffer_id, |buffer| {
                            buffer.finish_syntax_refresh(token.generation())
                        })
                        .unwrap_or(false)
                    {
                        accepted_redraw = true;
                    }
                }
                JobEvent::Completed {
                    payload: Some(JobPayload::IndentScopeRefresh(result)),
                    ..
                } => {
                    if self
                        .with_buffer_mut(result.buffer_id, |buffer| {
                            buffer.apply_indent_scope_refresh_result(result)
                        })
                        .unwrap_or(false)
                    {
                        accepted_redraw = true;
                    }
                }
                JobEvent::Completed {
                    payload: Some(JobPayload::DiffRefresh(result)),
                    ..
                } => {
                    if self
                        .with_buffer_mut(result.buffer_id, |buffer| {
                            buffer.apply_diff_refresh_result(result)
                        })
                        .unwrap_or(false)
                    {
                        accepted_redraw = true;
                    }
                }
                JobEvent::Completed { payload: None, .. } => {}
                JobEvent::Completed {
                    payload: Some(payload),
                    ..
                } => {
                    tracing::error!(?payload, "unexpected buffer-pool job payload");
                }
                JobEvent::Failed { kind, error, .. } => {
                    if let JobKind::DiffRefresh(buffer_id) = kind
                        && let Some(buffer) = self.buffers.get_mut(&buffer_id)
                    {
                        if buffer.generations.diff_background == Some(buffer.generations.diff) {
                            buffer.generations.diff_background = None;
                        }
                    }
                    tracing::warn!(?error, "buffer-pool job failed");
                }
                _ => {}
            }
        }

        accepted_redraw
    }

    /// Reloads clean file-backed buffers whose on-disk contents changed.
    pub fn process_external_file_changes(&mut self) -> bool {
        let now = Instant::now();
        if self
            .last_disk_check
            .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(1))
        {
            return false;
        }
        self.last_disk_check = Some(now);

        let mut accepted_redraw = false;

        for buffer_id in self.buffer_ids() {
            let mut should_refresh_cache = false;

            {
                let Some(buffer) = self.buffers.get_mut(&buffer_id) else {
                    continue;
                };

                if buffer.saved_disk_state.is_none() {
                    self.reported_external_conflicts.remove(&buffer_id);
                    continue;
                }

                let Some(path) = buffer.path().cloned() else {
                    continue;
                };

                let current_state = Buffer::disk_state_for_path(path.as_path());
                if current_state == buffer.saved_disk_state {
                    self.reported_external_conflicts.remove(&buffer_id);
                    continue;
                }

                if buffer.is_modified() {
                    if self.reported_external_conflicts.get(&buffer_id) != Some(&current_state) {
                        self.reported_external_conflicts
                            .insert(buffer_id, current_state);
                        globals::enqueue_editor_event(EditorEvent::ExternalFileConflict {
                            snapshot: BufferEventSnapshot::from_buffer(buffer_id, buffer),
                        });
                    }
                    continue;
                }

                if current_state.is_none() {
                    continue;
                }

                match buffer.reload_from_disk() {
                    Ok(()) => {
                        self.reported_external_conflicts.remove(&buffer_id);
                        globals::enqueue_editor_event(EditorEvent::BufferReloaded {
                            snapshot: BufferEventSnapshot::from_buffer(buffer_id, buffer),
                        });
                        should_refresh_cache = true;
                    }
                    Err(error) => {
                        tracing::warn!(?error, ?buffer_id, "failed to reload buffer from disk");
                    }
                }
            }

            if should_refresh_cache {
                self.request_buffer_cache_refresh(buffer_id);
                accepted_redraw = true;
            }
        }

        accepted_redraw
    }

    /// Returns true when the buffer should confirm overwriting newer on-disk contents.
    pub fn buffer_needs_overwrite_confirmation(&self, buffer_id: BufferId) -> bool {
        let Some(buffer) = self.buffers.get(&buffer_id) else {
            return false;
        };

        let Some(saved_state) = buffer.saved_disk_state else {
            return false;
        };

        let Some(current_state) = buffer.current_disk_state() else {
            return true;
        };

        current_state != saved_state
    }

    /// Warms the active buffer briefly, then queues asynchronous syntax refreshes for all buffers.
    pub fn request_syntax_refresh_at_startup(
        &mut self,
        active_buffer_id: Option<BufferId>,
        active_scroll_row: usize,
        visible_rows: usize,
        syntax_enabled: bool,
    ) {
        if !syntax_enabled {
            return;
        }

        if let Some(active_buffer_id) = active_buffer_id
            && visible_rows > 0
            && let Some(buffer) = self.buffers.get_mut(&active_buffer_id)
        {
            let target_line = active_scroll_row
                .saturating_add(visible_rows.saturating_sub(1))
                .saturating_add(32);
            buffer
                .warm_syntax_through_with_budget(target_line, std::time::Duration::from_millis(10));
        }

        for buffer_id in self.buffer_ids() {
            if !self.buffers.contains_key(&buffer_id) {
                continue;
            };

            self.request_buffer_cache_refresh(buffer_id);
        }
    }

    /// Saves the buffer using its stored path.
    ///
    /// Missing files are created on write if the buffer already has a resolved path.
    pub fn save_buffer(&mut self, id: BufferId) -> io::Result<()> {
        let path = self
            .buffers
            .get(&id)
            .and_then(|buffer| buffer.path().cloned());
        let result = match self.buffers.get_mut(&id) {
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "buffer id not found",
            )),
            Some(_) if path.is_none() => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "buffer has no path",
            )),
            Some(buffer) => {
                let path = path.as_ref().expect("validated path should exist");
                buffer
                    .save_to_file(path.as_path())
                    .map(|()| buffer.mark_saved())
            }
        };

        match result {
            Ok(()) => {
                self.reported_external_conflicts.remove(&id);
                let snapshot = BufferEventSnapshot::from_buffer(
                    id,
                    self.buffers.get(&id).expect("saved buffer should exist"),
                );
                globals::enqueue_editor_event(EditorEvent::BufferSaved { snapshot });
                Ok(())
            }
            Err(error) => {
                globals::enqueue_editor_event(EditorEvent::BufferSaveFailed {
                    error: BufferErrorSnapshot::from_io_error(
                        Some(id),
                        path.map(|path| path.as_path().to_path_buf()),
                        &error,
                    ),
                });
                Err(error)
            }
        }
    }

    /// Saves the buffer to an explicit path after resolving it to an absolute
    /// path.
    ///
    /// The destination file is created if it does not already exist.
    pub fn save_buffer_to_path(&mut self, id: BufferId, path: impl AsRef<Path>) -> io::Result<()> {
        let abs_path = match AbsolutePath::from_path(path.as_ref()) {
            Some(path) => path,
            None => {
                let error = io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "failed to resolve absolute path",
                );
                globals::enqueue_editor_event(EditorEvent::BufferSaveFailed {
                    error: BufferErrorSnapshot::from_io_error(Some(id), None, &error),
                });
                return Err(error);
            }
        };
        let Some(buffer) = self.buffers.get_mut(&id) else {
            let error = io::Error::new(io::ErrorKind::NotFound, "buffer id not found");
            globals::enqueue_editor_event(EditorEvent::BufferSaveFailed {
                error: BufferErrorSnapshot::from_io_error(
                    Some(id),
                    Some(abs_path.as_path().to_path_buf()),
                    &error,
                ),
            });
            return Err(error);
        };
        if let Err(error) = buffer.save_to_file(abs_path.as_path()) {
            globals::enqueue_editor_event(EditorEvent::BufferSaveFailed {
                error: BufferErrorSnapshot::from_io_error(
                    Some(id),
                    Some(abs_path.as_path().to_path_buf()),
                    &error,
                ),
            });
            return Err(error);
        }

        let old_path = buffer.path().cloned();
        if let Some(old_path) = &old_path {
            self.paths.remove(old_path);
        }
        buffer.set_path(abs_path.clone());
        buffer.mark_saved();
        self.paths.insert(abs_path, id);
        self.reported_external_conflicts.remove(&id);

        if old_path.as_ref() != buffer.path() {
            globals::enqueue_editor_event(EditorEvent::BufferPathChanged {
                snapshot: BufferPathChangeSnapshot {
                    buffer: BufferEventSnapshot::from_buffer(id, buffer),
                    previous_path: old_path.map(|path| path.as_path().to_path_buf()),
                },
            });
        }
        globals::enqueue_editor_event(EditorEvent::BufferSaved {
            snapshot: BufferEventSnapshot::from_buffer(id, buffer),
        });
        Ok(())
    }

    /// Saves every modified buffer that has a resolved path.
    pub fn save_modified_buffers(&mut self) -> io::Result<Vec<BufferId>> {
        let buffer_ids = self.modified_buffer_ids();
        let mut saved = Vec::new();

        for buffer_id in buffer_ids {
            let Some(buffer) = self.buffers.get(&buffer_id) else {
                continue;
            };

            if buffer.path().is_none() {
                continue;
            }

            self.save_buffer(buffer_id)?;
            saved.push(buffer_id);
        }

        Ok(saved)
    }

    /// Requests buffer cache refresh for a buffer using the pool-owned job engine.
    pub fn request_buffer_cache_refresh(&mut self, buffer_id: BufferId) {
        let (syntax_job, indent_job, diff_job, generation, diff_generation) = self
            .buffers
            .get_mut(&buffer_id)
            .and_then(|buffer| {
                let generation = buffer.syntax_generation();
                let syntax_needed = !buffer.syntax_cache_complete()
                    && buffer.generations.syntax_background != Some(generation);
                let indent_needed = buffer.indent_scope_cache_stale()
                    && buffer.generations.indent_background != Some(generation);
                let diff_generation = buffer.generations.diff;
                let diff_needed = buffer.diff_cache_stale()
                    && buffer.generations.diff_background != Some(diff_generation);

                if !syntax_needed && !indent_needed && !diff_needed {
                    return None;
                }

                let syntax_job = syntax_needed.then(|| {
                    SyntaxRefreshJob::new(
                        buffer_id,
                        generation,
                        buffer.syntax_name().to_owned().into(),
                        buffer.buffer_cache.syntax_cache.clone(),
                        buffer.lines.clone(),
                    )
                });
                let indent_job = indent_needed.then(|| {
                    IndentScopeRefreshJob::new(
                        buffer_id,
                        generation,
                        buffer.buffer_cache.indent_scope_cache.clone(),
                        buffer.lines.clone(),
                    )
                });

                let diff_job = if diff_needed {
                    buffer.path().cloned().map(|path| {
                        DiffRefreshJob::new(buffer_id, diff_generation, path, buffer.line_texts())
                    })
                } else {
                    None
                };

                Some((
                    syntax_job,
                    indent_job,
                    diff_job,
                    generation,
                    diff_generation,
                ))
            })
            .unwrap_or((None, None, None, 0, 0));

        if let Some(job) = syntax_job {
            let kind = JobKind::SyntaxRefresh(buffer_id);
            let token = JobToken::new(generation);
            if self.submit_background_job(kind, token, job).is_ok()
                && let Some(buffer) = self.buffers.get_mut(&buffer_id)
            {
                buffer.generations.syntax_background = Some(generation);
            }
        }

        if let Some(job) = indent_job {
            let kind = JobKind::IndentScopeRefresh(buffer_id);
            let token = JobToken::new(generation);
            if self.submit_background_job(kind, token, job).is_ok()
                && let Some(buffer) = self.buffers.get_mut(&buffer_id)
            {
                buffer.generations.indent_background = Some(generation);
            }
        }

        if let Some(job) = diff_job {
            let kind = JobKind::DiffRefresh(buffer_id);
            let token = JobToken::new(diff_generation);
            if self.submit_background_job(kind, token, job).is_ok()
                && let Some(buffer) = self.buffers.get_mut(&buffer_id)
            {
                buffer.generations.diff_background = Some(diff_generation);
            }
        }
    }

    /// Runs a closure with mutable access to a live buffer entry.
    ///
    /// The closure executes while the pool is locked, so edits are serialized
    /// and cannot escape as detached snapshots.
    pub fn with_buffer_mut<R>(
        &mut self,
        id: BufferId,
        f: impl FnOnce(&mut Buffer) -> R,
    ) -> Option<R> {
        let buffer = self.buffers.get_mut(&id)?;
        Some(f(buffer))
    }

    fn insert_buffer(&mut self, buffer: Buffer) -> BufferId {
        if let Some(path) = buffer.path().cloned()
            && let Some(id) = self.paths.get(&path).copied()
        {
            return id;
        }

        let id = BufferId::new(self.next_id);
        self.next_id += 1;
        if let Some(path) = buffer.path().cloned() {
            self.paths.insert(path, id);
        }
        self.buffers.insert(id, buffer);
        let snapshot = BufferEventSnapshot::from_buffer(id, &self.buffers[&id]);
        globals::enqueue_editor_event(EditorEvent::BufferLoaded { snapshot });
        id
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::config::Config;
    use crate::globals;
    use std::fs;
    use std::sync::{
        Arc, Barrier, Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    };
    use std::thread;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("urvim-buffer-pool-{}-{}", std::process::id(), name))
    }

    #[test]
    fn test_create_buffer_ids_increment_from_zero() {
        let mut pool = BufferPool::new();

        let first = pool.create_buffer();
        let second = pool.create_buffer();

        assert_eq!(first.get(), 0);
        assert_eq!(second.get(), 1);
    }

    #[test]
    fn test_open_buffer_deduplicates_absolute_path() {
        let path = temp_file("dedup.txt");
        fs::write(&path, "alpha").unwrap();

        let mut pool = BufferPool::new();
        let first = pool.open_buffer(&path).unwrap();
        let second = pool.open_buffer(&path).unwrap();

        assert_eq!(first, second);
        assert_eq!(pool.get(first).unwrap().as_str(), "alpha");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_open_buffer_missing_path_creates_empty_file_backed_buffer() {
        let path = temp_file("missing-backed.txt");
        let mut pool = BufferPool::new();

        let id = pool.open_buffer(&path).unwrap();
        let buffer = pool.get(id).unwrap();

        assert_eq!(buffer.as_str(), "");
        assert!(!buffer.is_modified());
        assert_eq!(
            buffer.path().map(|resolved| resolved.as_path()),
            Some(path.as_path())
        );

        let reopened = pool.open_buffer(&path).unwrap();
        assert_eq!(id, reopened);
    }

    #[test]
    fn test_save_buffer_creates_missing_file_for_file_backed_buffer() {
        let path = temp_file("save-creates.txt");
        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&path).unwrap();

        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(crate::buffer::Cursor::new(0, 0), "hello");
        })
        .unwrap();

        pool.save_buffer(id).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        assert!(!pool.get(id).unwrap().is_modified());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_process_external_file_changes_reloads_clean_buffer() {
        let path = temp_file("reload-clean.txt");
        fs::write(&path, "alpha").unwrap();

        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&path).unwrap();

        fs::write(&path, "alphabet").unwrap();

        assert!(pool.process_external_file_changes());
        assert_eq!(pool.get(id).unwrap().as_str(), "alphabet");
        assert!(!pool.get(id).unwrap().is_modified());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_process_external_file_changes_skips_modified_buffer() {
        let path = temp_file("reload-skip-modified.txt");
        fs::write(&path, "alpha").unwrap();

        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&path).unwrap();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(crate::buffer::Cursor::new(0, 5), "!");
        })
        .unwrap();

        fs::write(&path, "beta").unwrap();

        assert!(!pool.process_external_file_changes());
        assert_eq!(pool.get(id).unwrap().as_str(), "alpha!");
        assert!(pool.get(id).unwrap().is_modified());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_buffer_needs_overwrite_confirmation_detects_external_changes() {
        let path = temp_file("overwrite-confirm.txt");
        fs::write(&path, "alpha").unwrap();

        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&path).unwrap();

        assert!(!pool.buffer_needs_overwrite_confirmation(id));

        fs::write(&path, "alphabet").unwrap();

        assert!(pool.buffer_needs_overwrite_confirmation(id));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_modified_buffer_ids_include_hidden_buffers() {
        let path = temp_file("modified-hidden.txt");
        fs::write(&path, "alpha").unwrap();

        let mut pool = BufferPool::new();
        let visible = pool.open_buffer(&path).unwrap();
        let hidden = pool
            .create_buffer_with_path(&path.with_file_name("modified-hidden-2.txt"))
            .unwrap();
        pool.with_buffer_mut(hidden, |buffer| {
            buffer.insert_text(crate::buffer::Cursor::new(0, 0), "beta");
        })
        .unwrap();

        assert_eq!(pool.modified_buffer_count(), 1);
        assert_eq!(pool.modified_buffer_ids(), vec![hidden]);
        assert!(!pool.get(visible).unwrap().is_modified());

        fs::remove_file(&path).ok();
        fs::remove_file(path.with_file_name("modified-hidden-2.txt")).ok();
    }

    #[test]
    fn test_save_modified_buffers_writes_hidden_buffers() {
        let visible_path = temp_file("write-all-visible.txt");
        let hidden_path = temp_file("write-all-hidden.txt");
        fs::write(&visible_path, "alpha").unwrap();
        fs::write(&hidden_path, "gamma").unwrap();

        let mut pool = BufferPool::new();
        let visible = pool.open_buffer(&visible_path).unwrap();
        let hidden = pool.open_buffer(&hidden_path).unwrap();
        pool.with_buffer_mut(visible, |buffer| {
            buffer.insert_text(crate::buffer::Cursor::new(0, 5), "-1");
        })
        .unwrap();
        pool.with_buffer_mut(hidden, |buffer| {
            buffer.insert_text(crate::buffer::Cursor::new(0, 5), "-2");
        })
        .unwrap();

        let saved = pool.save_modified_buffers().unwrap();
        assert_eq!(saved, vec![visible, hidden]);
        assert_eq!(fs::read_to_string(&visible_path).unwrap(), "alpha-1");
        assert_eq!(fs::read_to_string(&hidden_path).unwrap(), "gamma-2");

        fs::remove_file(&visible_path).ok();
        fs::remove_file(&hidden_path).ok();
    }

    #[cfg(unix)]
    #[test]
    fn test_open_buffer_preserves_non_not_found_errors() {
        let parent = temp_file("blocked-parent");
        let blocked = parent.join("child.txt");
        let mut pool = BufferPool::new();

        fs::create_dir_all(&parent).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&parent).unwrap().permissions();
            permissions.set_mode(0o000);
            fs::set_permissions(&parent, permissions).unwrap();
        }

        let result = pool.open_buffer(&blocked);

        assert!(result.is_err());
        assert!(pool.paths.is_empty());
        assert!(pool.buffers.is_empty());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&parent).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&parent, permissions).unwrap();
        }
        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn test_with_buffer_mut_applies_changes_in_place() {
        let mut pool = BufferPool::new();
        let id = pool.create_buffer();

        let result = pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(crate::buffer::Cursor::new(0, 0), "alpha");
            buffer.as_str().to_string()
        });

        assert_eq!(result, Some("alpha".to_string()));
        assert_eq!(pool.get(id).unwrap().as_str(), "alpha");
    }

    #[test]
    fn test_with_buffer_mut_missing_buffer_returns_none() {
        let mut pool = BufferPool::new();

        let result = pool.with_buffer_mut(BufferId::new(999), |_buffer| ());

        assert!(result.is_none());
    }

    #[test]
    fn test_warmup_syntax_at_startup_warms_active_buffer_and_queues_hidden_buffers() {
        let _config_guard = globals::set_test_config(Config {
            theme: "demo".to_string(),
            syntax: true,
            auto_close_pairs: true,
            ..Default::default()
        });

        let mut pool = BufferPool::new();
        let active_id = pool.register_buffer(Buffer::from_str_with_path(
            "fn main() {\n    let value = 1;\n}",
            AbsolutePath::from_path(std::path::Path::new("/tmp/active.rs")).unwrap(),
        ));
        let hidden_id = pool.register_buffer(Buffer::from_str_with_path(
            "fn hidden() {\n    let value = 2;\n}",
            AbsolutePath::from_path(std::path::Path::new("/tmp/hidden.rs")).unwrap(),
        ));

        pool.request_syntax_refresh_at_startup(Some(active_id), 0, 2, true);

        let active = pool.get(active_id).expect("active buffer should exist");
        let hidden = pool.get(hidden_id).expect("hidden buffer should exist");

        assert!(active.cached_syntax_spans_for_line(0).is_some());
        assert!(!active.syntax_background_pending());
        assert!(hidden.syntax_background_pending());
        assert!(pool.buffer_ids().windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn test_with_buffer_mut_serializes_concurrent_edits() {
        let pool = Arc::new(Mutex::new(BufferPool::new()));
        let id = {
            let mut pool = pool.lock().unwrap();
            pool.create_buffer()
        };

        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();

        for ch in ['a', 'b'] {
            let pool = Arc::clone(&pool);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let mut pool = pool.lock().unwrap();
                pool.with_buffer_mut(id, |buffer| {
                    let cursor = crate::buffer::Cursor::new(0, buffer.as_str().len());
                    buffer.insert_char(cursor, ch);
                })
                .unwrap();
            }));
        }

        barrier.wait();
        for handle in handles {
            handle.join().unwrap();
        }

        let pool = pool.lock().unwrap();
        let buffer = pool.get(id).unwrap();
        assert_eq!(buffer.as_str().len(), 2);
        assert!(buffer.as_str().contains('a'));
        assert!(buffer.as_str().contains('b'));
    }

    #[test]
    fn test_rwlock_allows_multiple_concurrent_readers() {
        let pool = Arc::new(RwLock::new(BufferPool::new()));
        let id = {
            let mut pool = pool.write().unwrap();
            pool.create_buffer()
        };

        let barrier = Arc::new(Barrier::new(3));
        let active_readers = Arc::new(AtomicUsize::new(0));
        let peak_readers = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..2 {
            let pool = Arc::clone(&pool);
            let barrier = Arc::clone(&barrier);
            let active_readers = Arc::clone(&active_readers);
            let peak_readers = Arc::clone(&peak_readers);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let pool = pool.read().unwrap();
                let current = active_readers.fetch_add(1, Ordering::SeqCst) + 1;
                peak_readers.fetch_max(current, Ordering::SeqCst);
                assert_eq!(pool.get(id).unwrap().line_count(), 1);
                barrier.wait();
                active_readers.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        barrier.wait();
        barrier.wait();
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(peak_readers.load(Ordering::SeqCst), 2);
    }

    fn drain_loaded_events() -> Vec<BufferId> {
        let mut ids = Vec::new();
        let mut retained = Vec::new();
        while let Some(event) = globals::take_editor_event() {
            match event {
                crate::event::EditorEvent::BufferLoaded { snapshot } => {
                    ids.push(snapshot.buffer_id)
                }
                event => retained.push(event),
            }
        }
        for event in retained {
            globals::enqueue_editor_event(event);
        }
        ids
    }

    fn drain_unloaded_events() -> Vec<(BufferId, crate::event::BufferEventSnapshot)> {
        let mut ids = Vec::new();
        let mut retained = Vec::new();
        while let Some(event) = globals::take_editor_event() {
            match event {
                crate::event::EditorEvent::BufferUnloaded { snapshot } => {
                    ids.push((snapshot.buffer_id, snapshot))
                }
                event => retained.push(event),
            }
        }
        for event in retained {
            globals::enqueue_editor_event(event);
        }
        ids
    }

    fn drain_editor_events() -> Vec<crate::event::EditorEvent> {
        std::iter::from_fn(globals::take_editor_event).collect()
    }

    #[test]
    fn test_save_as_emits_path_changed_before_saved_once() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("save-as-events.txt");
        fs::remove_file(&path).ok();
        let mut pool = BufferPool::new();
        let id = pool.create_buffer();
        drain_editor_events();

        pool.save_buffer_to_path(id, &path).unwrap();

        let events = drain_editor_events();
        assert!(matches!(
            events.as_slice(),
            [
                crate::event::EditorEvent::BufferPathChanged { snapshot },
                crate::event::EditorEvent::BufferSaved { snapshot: saved }
            ] if snapshot.buffer.buffer_id == id
                && snapshot.previous_path.is_none()
                && saved.buffer_id == id
        ));
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_failed_save_as_keeps_path_and_emits_one_failure() {
        globals::clear_editor_events_for_tests();
        let original = temp_file("save-as-original.txt");
        fs::write(&original, "alpha").unwrap();
        let destination = temp_file("missing-parent").join("save-as.txt");
        fs::remove_dir_all(destination.parent().unwrap()).ok();
        fs::remove_file(destination.parent().unwrap()).ok();
        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&original).unwrap();
        drain_editor_events();

        assert!(pool.save_buffer_to_path(id, &destination).is_err());

        assert_eq!(pool.get(id).unwrap().path().unwrap().as_path(), original);
        let events = drain_editor_events();
        assert!(matches!(
            events.as_slice(),
            [crate::event::EditorEvent::BufferSaveFailed { error }]
                if error.buffer_id == Some(id)
                    && error.path.as_deref() == Some(destination.as_path())
                    && error.error_kind == "not_found"
        ));
        fs::remove_file(original).ok();
    }

    #[test]
    fn test_external_conflict_is_emitted_once_per_disk_state() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("external-conflict-events.txt");
        fs::write(&path, "alpha").unwrap();
        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&path).unwrap();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(crate::buffer::Cursor::new(0, 5), "!");
        });
        drain_editor_events();

        fs::write(&path, "longer beta").unwrap();
        assert!(!pool.process_external_file_changes());
        pool.last_disk_check = None;
        assert!(!pool.process_external_file_changes());
        assert_eq!(
            drain_editor_events()
                .iter()
                .filter(|event| matches!(
                    event,
                    crate::event::EditorEvent::ExternalFileConflict { .. }
                ))
                .count(),
            1
        );

        pool.last_disk_check = None;
        fs::write(&path, "a distinct, longer disk state").unwrap();
        assert!(!pool.process_external_file_changes());
        assert!(matches!(
            drain_editor_events().as_slice(),
            [crate::event::EditorEvent::ExternalFileConflict { snapshot }] if snapshot.buffer_id == id
        ));
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_clean_external_change_emits_reloaded_after_success() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("reload-event.txt");
        fs::write(&path, "alpha").unwrap();
        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&path).unwrap();
        drain_editor_events();

        fs::write(&path, "alphabet").unwrap();
        assert!(pool.process_external_file_changes());

        assert!(matches!(
            drain_editor_events().as_slice(),
            [crate::event::EditorEvent::BufferReloaded { snapshot }] if snapshot.buffer_id == id && !snapshot.modified
        ));
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_open_directory_emits_open_failed_with_absolute_path() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("open-directory-error");
        fs::remove_dir_all(&path).ok();
        fs::create_dir(&path).unwrap();
        let mut pool = BufferPool::new();

        let error = pool.open_buffer(&path).unwrap_err();

        let events = drain_editor_events();
        assert!(matches!(
            events.as_slice(),
            [crate::event::EditorEvent::BufferOpenFailed { error: snapshot }]
                if snapshot.path.as_deref() == Some(path.as_path())
                    && snapshot.message == error.to_string()
        ));
        fs::remove_dir(path).ok();
    }

    #[test]
    fn test_rename_buffer_path_emits_path_changed_after_success() {
        globals::clear_editor_events_for_tests();
        let old_path = temp_file("rename-old.txt");
        let new_path = temp_file("rename-new.txt");
        let mut pool = BufferPool::new();
        let id = pool.create_buffer_with_path(&old_path).unwrap();
        drain_editor_events();

        pool.rename_buffer_path(id, &new_path).unwrap();

        assert!(matches!(
            drain_editor_events().as_slice(),
            [crate::event::EditorEvent::BufferPathChanged { snapshot }]
                if snapshot.buffer.buffer_id == id
                    && snapshot.buffer.path.as_deref() == Some(new_path.as_path())
                    && snapshot.previous_path.as_deref() == Some(old_path.as_path())
        ));
    }

    #[test]
    fn test_create_buffer_enqueues_buffer_loaded_event() {
        globals::clear_editor_events_for_tests();
        let mut pool = BufferPool::new();
        let id = pool.create_buffer();
        let loaded = drain_loaded_events();
        assert_eq!(loaded, vec![id]);
    }

    #[test]
    fn test_create_buffer_with_path_enqueues_buffer_loaded_for_new_path() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("pool-new-path.txt");
        let mut pool = BufferPool::new();
        let id = pool.create_buffer_with_path(&path).unwrap();
        let loaded = drain_loaded_events();
        assert_eq!(loaded, vec![id]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_create_buffer_with_path_does_not_enqueue_when_reusing_path() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("pool-reuse-path.txt");
        let mut pool = BufferPool::new();
        let first = pool.create_buffer_with_path(&path).unwrap();
        drain_loaded_events();
        let reused = pool.create_buffer_with_path(&path).unwrap();
        assert_eq!(first, reused);
        assert!(drain_loaded_events().is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_open_buffer_enqueues_buffer_loaded_for_new_path() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("pool-open-new.txt");
        std::fs::write(&path, "alpha").unwrap();
        let mut pool = BufferPool::new();
        let id = pool.open_buffer(&path).unwrap();
        let loaded = drain_loaded_events();
        assert_eq!(loaded, vec![id]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_open_buffer_does_not_enqueue_when_reusing_path() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("pool-open-reuse.txt");
        std::fs::write(&path, "alpha").unwrap();
        let mut pool = BufferPool::new();
        let first = pool.open_buffer(&path).unwrap();
        drain_loaded_events();
        let reused = pool.open_buffer(&path).unwrap();
        assert_eq!(first, reused);
        assert!(drain_loaded_events().is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_remove_buffer_enqueues_buffer_unloaded_with_snapshot() {
        globals::clear_editor_events_for_tests();
        let path = temp_file("pool-unload.txt");
        let mut pool = BufferPool::new();
        let id = pool.create_buffer_with_path(&path).unwrap();
        drain_loaded_events();

        assert!(pool.remove_buffer(id).is_some());
        let unloaded = drain_unloaded_events();
        assert_eq!(unloaded.len(), 1);
        let (event_id, snapshot) = &unloaded[0];
        assert_eq!(*event_id, id);
        assert_eq!(snapshot.buffer_id, id);
        assert_eq!(snapshot.path.as_deref(), Some(path.as_path()));
        assert!(!snapshot.modified);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_remove_buffer_does_not_enqueue_for_missing_id() {
        globals::clear_editor_events_for_tests();
        let mut pool = BufferPool::new();
        assert!(pool.remove_buffer(BufferId::new(999)).is_none());
        assert!(drain_unloaded_events().is_empty());
    }
}
