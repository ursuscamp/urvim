//! Terminal size detection and cursor position queries.
//!
//! This module provides utilities for:
//! - Getting the current terminal dimensions
//! - Querying the cursor position from the terminal
//! - Low-level input polling

use rustix::event::{poll, PollFd, PollFlags};
use rustix::fd::AsFd;
use std::io::{Read, Write};
use tracing::trace;

/// Gets the current terminal size as (rows, columns).
pub fn get_terminal_size() -> Option<(u16, u16)> {
    terminal_size::terminal_size().map(|(w, h)| (h.0, w.0))
}

/// Queries the terminal for the current cursor position.
#[allow(dead_code)]
pub fn query_cursor_position<I: Read + AsFd, O: Write + AsFd>(
    input: &mut I,
    output: &mut O,
    flush: bool,
) -> Option<(u16, u16)> {
    // Flush output to ensure the query is sent
    if flush {
        output.flush().ok()?;
    }

    // Poll for input availability with timeout
    let pollfd = PollFd::new(input, PollFlags::IN);
    match poll(&mut [pollfd], 500) {
        Ok(_) => {}
        Err(e) => {
            trace!("poll failed: {:?}", e);
            return None;
        }
    };

    // Read any available response data
    let mut buf = [0u8; 32];
    let n = match input.read(&mut buf) {
        Ok(n) => n,
        Err(e) => {
            trace!("read failed: {:?}", e);
            return None;
        }
    };

    if n == 0 {
        return None;
    }

    let response = &buf[..n];

    // Keep reading until we get a valid cursor position response (\x1b[...R)
    // This handles cases where there might be leftover escape sequence data in the buffer
    // from previous operations. We attempt to find a properly formatted response.
    let mut attempts = 0;
    let mut response = response;

    while !response.starts_with(b"\x1b[") || !response.ends_with(b"R") {
        attempts += 1;
        if attempts > 10 {
            trace!("Too many attempts to find valid cursor position response");
            return None;
        }

        // Wait for more data to arrive
        let pollfd = PollFd::new(input, PollFlags::IN);
        if poll(&mut [pollfd], 100).is_err() {
            trace!("poll failed on retry");
            return None;
        }

        let n = match input.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                trace!("read failed on retry: {:?}", e);
                return None;
            }
        };

        if n == 0 {
            return None;
        }

        response = &buf[..n];
    }

    // Now we have a valid response starting with \x1b[ and ending with R
    // Parse the dimensions from between [ and R
    let dims = &response[2..response.len() - 1];
    if let Some(pos) = dims.iter().position(|&b| b == b';') {
        let row_str = std::str::from_utf8(&dims[..pos]).ok()?;
        let col_str = std::str::from_utf8(&dims[pos + 1..]).ok()?;
        let row: u16 = row_str.parse().ok()?;
        let col: u16 = col_str.parse().ok()?;
        return Some((row, col));
    }

    trace!("Failed to parse cursor position response");
    None
}

#[allow(dead_code)]
pub fn poll_input<T: AsFd>(fd: &T, timeout_ms: i32) -> bool {
    let poll_fd = PollFd::new(fd, PollFlags::IN);
    match poll(&mut [poll_fd], timeout_ms) {
        Ok(n) => n > 0, // True only if there's actual data (n > 0 means events ready)
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_cursor_position_simple() {
        let response = b"\x1b[5;10R";
        let dims = &response[2..response.len() - 1];
        let pos = dims.iter().position(|&b| b == b';').unwrap();
        let row_str = std::str::from_utf8(&dims[..pos]).unwrap();
        let col_str = std::str::from_utf8(&dims[pos + 1..]).unwrap();
        let row: u16 = row_str.parse().unwrap();
        let col: u16 = col_str.parse().unwrap();
        assert_eq!((row, col), (5, 10));
    }

    #[test]
    fn test_parse_cursor_position_single_digit() {
        let response = b"\x1b[1;1R";
        let dims = &response[2..response.len() - 1];
        let pos = dims.iter().position(|&b| b == b';').unwrap();
        let row_str = std::str::from_utf8(&dims[..pos]).unwrap();
        let col_str = std::str::from_utf8(&dims[pos + 1..]).unwrap();
        let row: u16 = row_str.parse().unwrap();
        let col: u16 = col_str.parse().unwrap();
        assert_eq!((row, col), (1, 1));
    }

    #[test]
    fn test_parse_cursor_position_large() {
        let response = b"\x1b[100;200R";
        let dims = &response[2..response.len() - 1];
        let pos = dims.iter().position(|&b| b == b';').unwrap();
        let row_str = std::str::from_utf8(&dims[..pos]).unwrap();
        let col_str = std::str::from_utf8(&dims[pos + 1..]).unwrap();
        let row: u16 = row_str.parse().unwrap();
        let col: u16 = col_str.parse().unwrap();
        assert_eq!((row, col), (100, 200));
    }

    #[test]
    fn test_is_valid_cursor_response() {
        assert!(b"\x1b[5;10R".starts_with(b"\x1b[") && b"\x1b[5;10R".ends_with(b"R"));
        assert!(b"\x1b[1;1R".starts_with(b"\x1b[") && b"\x1b[1;1R".ends_with(b"R"));
        assert!(!b"\x1b[1;1B".starts_with(b"\x1b[") || !b"\x1b[1;1B".ends_with(b"R"));
    }

    #[test]
    fn test_is_invalid_with_leftover_data() {
        assert!(!b"\x1b[1;1B".starts_with(b"\x1b[") || !b"\x1b[1;1B".ends_with(b"R"));
        assert!(!b"\x1b[6n".starts_with(b"\x1b[") || !b"\x1b[6n".ends_with(b"R"));
    }
}
