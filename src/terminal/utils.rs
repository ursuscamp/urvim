//! Terminal utility functions.
//!
//! This module provides miscellaneous terminal-related utilities,
//! including clipboard operations via the OSC 52 protocol.

/// Writes a number as decimal digits to a buffer.
///
/// Returns the new buffer index after writing. The digits are written
/// in reverse order (least significant first), then reversed in the buffer.
///
/// # Arguments
///
/// * `n` - The number to write
/// * `buf` - The buffer to write to
/// * `i` - The starting index in the buffer
///
/// # Example
///
/// ```
/// let mut buf = [0u8; 10];
/// let i = write_decimal(123u32, &mut buf, 0);
/// assert_eq!(&buf[..i], b"123");
/// ```
pub fn write_decimal<N: TryInto<u64>>(n: N, buf: &mut [u8], mut i: usize) -> usize {
    let n = match n.try_into() {
        Ok(n) => n,
        Err(_) => {
            buf[i] = b'0';
            return i + 1;
        }
    };
    if n == 0 {
        buf[i] = b'0';
        return i + 1;
    }
    let mut digits = [0u8; 20];
    let mut len = 0;
    let mut n = n;
    while n > 0 {
        digits[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for j in (0..len).rev() {
        buf[i] = digits[j];
        i += 1;
    }
    i
}

#[allow(dead_code)]
/// Copies text to the system clipboard using OSC 52.
///
/// This sends an Operating System Command (OSC) 52 sequence to the terminal,
/// which forwards the text to the system clipboard. This is supported by
/// most modern terminals (iTerm2, tmux, screen, Windows Terminal, etc.).
///
/// # Arguments
///
/// * `text` - The text to copy to the clipboard
///
/// # Returns
///
/// A byte vector containing the OSC 52 escape sequence, which should be
/// written to the terminal.
///
/// # Example
///
/// ```
/// use terminal::utils::osc52_copy_to_clipboard;
///
/// let seq = osc52_copy_to_clipboard("Hello, world!");
/// // seq contains: \x1b]52;c;SGVsbG8sIHdvcmxkIT\x07
/// ```
pub fn osc52_copy_to_clipboard(text: &str) -> Vec<u8> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let mut seq = Vec::with_capacity(8 + encoded.len());
    seq.extend_from_slice(b"\x1b]52;c;");
    seq.extend_from_slice(encoded.as_bytes());
    seq.push(0x07);
    seq
}
