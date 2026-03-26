//! Global buffer pool and buffer identifiers.
//!
//! The buffer pool owns all live buffers in the editor and assigns each one a
//! stable `BufferId`. It also deduplicates file-backed buffers by absolute
//! path so opening the same file twice reuses the existing in-memory buffer.
//! Mutable access runs through the pool while the pool is locked so edits stay
//! synchronized across threads.

use super::Buffer;
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
            io::Error::new(io::ErrorKind::InvalidInput, "failed to resolve absolute path")
        })?;

        if let Some(id) = self.paths.get(&abs_path).copied() {
            return Ok(id);
        }

        Ok(self.insert_buffer(Buffer::with_path(abs_path)))
    }

    /// Opens a file-backed buffer from a path, reusing an existing buffer if the
    /// same absolute path is already present in the pool.
    pub fn open_buffer(&mut self, path: impl AsRef<Path>) -> io::Result<BufferId> {
        let abs_path = AbsolutePath::from_path(path.as_ref()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "failed to resolve absolute path")
        })?;

        if let Some(id) = self.paths.get(&abs_path).copied() {
            return Ok(id);
        }

        let mut buffer = Buffer::load_from_file(abs_path.as_path())?;
        buffer.set_path(abs_path.clone());
        Ok(self.insert_buffer(buffer))
    }

    /// Returns an immutable reference to a buffer by ID.
    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.get(&id)
    }

    /// Returns a mutable reference to a buffer by ID.
    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.buffers.get_mut(&id)
    }

    /// Saves the buffer using its stored path.
    pub fn save_buffer(&mut self, id: BufferId) -> io::Result<()> {
        let path = self
            .buffers
            .get(&id)
            .and_then(|buffer| buffer.path().cloned())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "buffer has no path"))?;
        let buffer = self
            .buffers
            .get(&id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "buffer id not found"))?;
        buffer.save_to_file(path.as_path())
    }

    /// Saves the buffer to an explicit path after resolving it to an absolute
    /// path.
    pub fn save_buffer_to_path(&mut self, id: BufferId, path: impl AsRef<Path>) -> io::Result<()> {
        let abs_path = AbsolutePath::from_path(path.as_ref()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "failed to resolve absolute path")
        })?;
        let buffer = self
            .buffers
            .get_mut(&id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "buffer id not found"))?;
        buffer.set_path(abs_path.clone());
        let result = buffer.save_to_file(abs_path.as_path());
        if result.is_ok() {
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
    use std::fs;
    use std::sync::{atomic::{AtomicUsize, Ordering}, Arc, Barrier, Mutex, RwLock};
    use std::thread;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "urvim-buffer-pool-{}-{}",
            std::process::id(),
            name
        ))
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
    fn test_open_buffer_failure_does_not_create_entry() {
        let path = temp_file("missing.txt");
        let mut pool = BufferPool::new();

        let result = pool.open_buffer(&path);

        assert!(result.is_err());
        assert!(pool.paths.is_empty());
        assert!(pool.buffers.is_empty());
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
