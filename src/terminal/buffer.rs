//! Terminal buffer management for parsing input bytes.
//!
//! This module provides a custom byte buffer implementation optimized for
//! terminal input parsing. It uses a simple position-based model where
//! data is consumed from the front without shifting the underlying buffer.

const INITIAL_BUFFER_CAPACITY: usize = 128;

/// A byte buffer for storing and consuming terminal input bytes.
///
/// This buffer is designed for parsing escape sequences and other terminal
/// input. It maintains a position pointer that advances as data is consumed,
/// avoiding the need to shift buffer contents.
pub struct ByteBuffer {
    buffer: Vec<u8>,
    len: usize,
    pos: usize,
}

impl Default for ByteBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteBuffer {
    /// Creates a new empty byte buffer with default capacity.
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(INITIAL_BUFFER_CAPACITY),
            len: 0,
            pos: 0,
        }
    }

    /// Returns true if the buffer is empty (no bytes available for reading).
    pub fn is_empty(&self) -> bool {
        self.filled_len() == 0
    }

    /// Returns the number of bytes available for reading (filled length - position).
    ///
    /// This is the number of bytes that can be consumed from the current position.
    pub fn filled_len(&self) -> usize {
        self.len - self.pos
    }

    /// Returns a copy of the byte at the current position, if any.
    pub fn peek_byte(&self) -> Option<u8> {
        self.buffer.get(self.pos).copied()
    }

    /// Returns a slice of up to `n` bytes starting from the current position.
    pub fn peek_n(&self, n: usize) -> Option<&[u8]> {
        self.buffer.get(self.pos..).and_then(|s| s.get(..n))
    }

    /// Advances the position by `n` bytes, marking them as consumed.
    ///
    /// This does not remove the data from the buffer; it simply moves
    /// the read position forward. When the position reaches the end,
    /// the buffer should be cleared.
    pub fn consume(&mut self, n: usize) {
        self.pos += n;
    }

    /// Clears all data from the buffer, resetting position and length.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.len = 0;
        self.pos = 0;
    }

    /// Appends a single byte to the end of the buffer.
    ///
    /// If there is capacity in the underlying buffer, the byte is written
    /// directly. Otherwise, it is pushed, causing potential reallocation.
    pub fn push(&mut self, byte: u8) {
        if self.len < self.buffer.len() {
            self.buffer[self.len] = byte;
        } else {
            self.buffer.push(byte);
        }
        self.len += 1;
    }

    /// Extends the buffer with a slice of bytes.
    pub fn extend(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.push(b);
        }
    }

    /// Returns a slice of bytes from `start` to `end` indices.
    ///
    /// Returns an empty slice if the range is invalid.
    pub fn get_range(&self, start: usize, end: usize) -> &[u8] {
        self.buffer.get(start..end).unwrap_or(&[])
    }

    /// Returns the total number of bytes in the buffer.
    ///
    /// This includes both consumed and unconsumed bytes.
    pub fn len(&self) -> usize {
        self.len
    }
}
