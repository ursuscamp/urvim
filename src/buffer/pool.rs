//! Global buffer pool and buffer identifiers.
//!
//! The buffer pool owns all live buffers in the editor and assigns each one a
//! stable `BufferId`. It also deduplicates file-backed buffers by absolute
//! path so opening the same file twice reuses the existing in-memory buffer.
//! Mutable access runs through the pool while the pool is locked so edits stay
//! synchronized across threads.

use super::Buffer;
use crate::job::JobPriority;
use crate::path::AbsolutePath;
use std::collections::HashMap;
use std::io;
use std::path::Path;

/// Stable identifier for a buffer stored in the global buffer pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(usize);

impl BufferId {
    /// Creates a new buffer identifier from a raw numeric value.
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric identifier.
    pub fn get(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub struct BufferPool {
    next_id: usize,
    buffers: HashMap<BufferId, Buffer>,
    paths: HashMap<AbsolutePath, BufferId>,
}

impl BufferPool {
    /// Creates an empty buffer pool.
    pub fn new() -> Self {
        Self {
            next_id: 0,
            buffers: HashMap::new(),
            paths: HashMap::new(),
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
        let abs_path = AbsolutePath::from_path(path.as_ref()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "failed to resolve absolute path",
            )
        })?;

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
            Err(error) => Err(error),
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

    /// Returns all buffer identifiers currently loaded into the pool.
    pub fn buffer_ids(&self) -> Vec<BufferId> {
        let mut ids = self.buffers.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    }

    /// Warms syntax for all loaded buffers at startup using a visible/hidden split.
    pub fn warmup_syntax_at_startup(
        &mut self,
        active_buffer_id: Option<BufferId>,
        active_scroll_row: usize,
        visible_rows: usize,
        syntax_enabled: bool,
    ) {
        if !syntax_enabled {
            return;
        }

        let active_prefix_end = visible_rows.saturating_sub(1);

        for buffer_id in self.buffer_ids() {
            let Some(buffer) = self.buffers.get_mut(&buffer_id) else {
                continue;
            };

            if Some(buffer_id) == active_buffer_id {
                if active_scroll_row == 0 && visible_rows > 0 {
                    buffer.ensure_syntax_through(active_prefix_end);
                }
                buffer
                    .request_buffer_cache_refresh_with_priority(buffer_id, JobPriority::Foreground);
            } else {
                buffer.request_buffer_cache_refresh(buffer_id);
            }
        }
    }

    /// Saves the buffer using its stored path.
    ///
    /// Missing files are created on write if the buffer already has a resolved path.
    pub fn save_buffer(&mut self, id: BufferId) -> io::Result<()> {
        let buffer = self
            .buffers
            .get_mut(&id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "buffer id not found"))?;
        let path = buffer
            .path()
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "buffer has no path"))?;
        buffer.save_to_file(path.as_path())?;
        buffer.mark_saved();
        Ok(())
    }

    /// Saves the buffer to an explicit path after resolving it to an absolute
    /// path.
    ///
    /// The destination file is created if it does not already exist.
    pub fn save_buffer_to_path(&mut self, id: BufferId, path: impl AsRef<Path>) -> io::Result<()> {
        let abs_path = AbsolutePath::from_path(path.as_ref()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "failed to resolve absolute path",
            )
        })?;
        let buffer = self
            .buffers
            .get_mut(&id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "buffer id not found"))?;
        let result = buffer.save_to_file(abs_path.as_path());
        if result.is_ok() {
            buffer.set_path(abs_path.clone());
            buffer.mark_saved();
            self.paths.insert(abs_path, id);
        }
        result
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
    use crate::job::JobManager;
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
            insert_escape: None,
            syntax: true,
            auto_close_pairs: true,
            ..Default::default()
        });
        globals::set_job_manager(JobManager::new());

        let mut pool = BufferPool::new();
        let active_id = pool.register_buffer(Buffer::from_str_with_path(
            "fn main() {\n    let value = 1;\n}",
            AbsolutePath::from_path(std::path::Path::new("/tmp/active.rs")).unwrap(),
        ));
        let hidden_id = pool.register_buffer(Buffer::from_str_with_path(
            "fn hidden() {\n    let value = 2;\n}",
            AbsolutePath::from_path(std::path::Path::new("/tmp/hidden.rs")).unwrap(),
        ));

        pool.warmup_syntax_at_startup(Some(active_id), 0, 2, true);

        let active = pool.get(active_id).expect("active buffer should exist");
        let hidden = pool.get(hidden_id).expect("hidden buffer should exist");

        assert!(active.cached_syntax_spans_for_line(0).is_some());
        assert!(active.syntax_background_pending());
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
}
