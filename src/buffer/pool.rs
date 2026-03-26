//! Global buffer pool and buffer identifiers.
//!
//! The buffer pool owns all live buffers in the editor and assigns each one a
//! stable `BufferId`. It also deduplicates file-backed buffers by absolute
//! path so opening the same file twice reuses the existing in-memory buffer.

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

/// A temporary mutable buffer view that commits changes back to the pool when
/// dropped.
///
/// This lets existing edit code keep using familiar `Buffer` methods while the
/// actual storage remains in the global pool.
#[derive(Debug)]
pub struct BufferMutGuard {
    buffer_id: BufferId,
    buffer: Buffer,
}

impl BufferMutGuard {
    /// Creates a guard from an existing buffer snapshot.
    pub fn from_buffer(buffer_id: BufferId, buffer: Buffer) -> Self {
        Self { buffer_id, buffer }
    }
}

impl std::ops::Deref for BufferMutGuard {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl std::ops::DerefMut for BufferMutGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Drop for BufferMutGuard {
    fn drop(&mut self) {
        let buffer = std::mem::take(&mut self.buffer);
        crate::globals::with_buffer_pool(|pool| {
            pool.replace_buffer(self.buffer_id, buffer);
        });
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

    /// Replaces the buffer contents for an existing ID and updates path indexes
    /// as needed.
    pub fn replace_buffer(&mut self, id: BufferId, buffer: Buffer) {
        if let Some(existing) = self.buffers.insert(id, buffer) {
            if let Some(path) = existing.path() {
                if self.paths.get(path).is_some_and(|existing_id| *existing_id == id) {
                    self.paths.remove(path);
                }
            }
        }

        if let Some(path) = self.buffers.get(&id).and_then(|buffer| buffer.path().cloned()) {
            self.paths.insert(path, id);
        }
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

    /// Creates a mutable guard that will write changes back to the pool when it
    /// is dropped.
    pub fn guard(&self, id: BufferId) -> Option<BufferMutGuard> {
        self.buffers.get(&id).cloned().map(|buffer| BufferMutGuard {
            buffer_id: id,
            buffer,
        })
    }

    fn insert_buffer(&mut self, buffer: Buffer) -> BufferId {
        if let Some(path) = buffer.path().cloned() {
            if let Some(id) = self.paths.get(&path).copied() {
                return id;
            }
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
}
