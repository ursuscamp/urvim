//! Terminal escape sequence parsing.
//!
//! This module provides functions for parsing terminal escape sequences into
//! structured events. It handles multiple escape sequence formats:
//!
//! - **CSI (Control Sequence Introducer)**: `\x1b[...` - Most common format
//!   - Arrow keys: `\x1b[A`, `\x1b[B`, `\x1b[C`, `\x1b[D`
//!   - Function keys: `\x1b[P`, `\x1b[Q`, etc.
//!   - Home/End: `\x1b[H`, `\x1b[F`
//!   - CSI with tilde: `\x1b[1~`, `\x1b[2~`, etc.
//!
//! - **CSI-U (Kitty Keyboard Protocol)**: `\x1b[...u` - Modern format with modifier support
//!   - Supports explicit modifier encoding: `\x1b[97;5u` = Ctrl+a
//!   - Key codes 1-27 for special keys, 33-126 for characters
//!
//! - **SS3 (Single Shift 3)**: `\x1bO...` - Old format for function keys
//!   - `\x1bOP`, `\x1bOQ`, `\x1bOR`, `\x1bOS`
//!
//! - **DSR (Device Status Report)**: `\x1b[rows;colsR` - Cursor position response
//!
//! The parsing attempts each format in order and returns the first successful match.

use crate::terminal::buffer::ByteBuffer;
use crate::terminal::keys::{Event, KeyCode, Modifiers};

/// Parses a complete event from the byte buffer.
///
/// This is the main entry point for parsing terminal input. It examines
/// the first byte to determine the type of input:
/// - Control characters (0-31): converted to Ctrl+letter
/// - ASCII printable (32-127): converted to KeyCode::Char
/// - High bytes (128+): UTF-8 multi-byte sequences
/// - Escape character (0x1b): delegate to escape sequence parser
///
/// Returns an `Event::Key` with the parsed key, or `Event::Resize` for
/// cursor position reports.
pub fn parse_event_with_buffer(buf: &mut ByteBuffer) -> Event {
    let first = buf.peek_byte().unwrap_or(0);
    tracing::debug!("parse_event: first byte=0x{:02x}", first);
    match first {
        // Null byte (empty input)
        0 => {
            buf.consume(1);
            KeyCode::Null.event()
        }
        // Escape character - start of escape sequence
        b'\x1b' => {
            let data = buf.peek_n(buf.filled_len()).unwrap_or(&[]);
            tracing::debug!("escape sequence: {:02x?}", data);
            buf.consume(1);
            parse_escape_seq_with_buffer(buf)
        }
        // Newline/Return - normalize to Enter
        b'\n' | b'\r' => {
            buf.consume(1);
            KeyCode::Enter.event()
        }
        // Tab character
        b'\t' => {
            buf.consume(1);
            KeyCode::Tab.event()
        }
        // Delete/Backspace (0x7f)
        b'\x7f' => {
            tracing::debug!("got 0x7f (DEL/Backspace)");
            buf.consume(1);
            KeyCode::Backspace.event()
        }
        // Backspace (0x08) - some terminals send this
        0x08 => {
            tracing::debug!("got 0x08 (Backspace)");
            buf.consume(1);
            KeyCode::Backspace.event()
        }
        // Control characters (1-31) - convert to Ctrl+letter
        // e.g., 0x01 -> Ctrl+A, 0x03 -> Ctrl+C
        b if b < 32 => {
            buf.consume(1);
            KeyCode::Char((b + 96) as char)
                .with_modifiers(Modifiers::CTRL)
                .event()
        }
        // ASCII printable characters
        b if b < 128 => {
            buf.consume(1);
            KeyCode::Char(b as char).event()
        }
        // High bytes (128+) - UTF-8 multi-byte sequence
        b => {
            let data = buf.peek_n(buf.filled_len()).unwrap_or(&[]);

            if let Ok(s) = std::str::from_utf8(data)
                && let Some(c) = s.chars().next()
            {
                let char_len = c.len_utf8();
                buf.consume(char_len);
                return KeyCode::Char(c).event();
            }

            tracing::warn!("Invalid UTF-8 sequence, skipping byte: {:02x}", b);
            buf.consume(1);
            KeyCode::Null.event()
        }
    }
}

/// Parses an escape sequence after the initial escape character has been consumed.
///
/// The buffer should NOT contain the leading escape character (0x1b) when
/// this function is called. It handles:
/// - CSI sequences (starting with '[')
/// - SS3 sequences (starting with 'O')
/// - Nested escape sequences (double escape)
///
/// If the sequence cannot be parsed, returns `KeyCode::Esc.event()`.
fn parse_escape_seq_with_buffer(buf: &mut ByteBuffer) -> Event {
    let data = buf.peek_n(buf.filled_len()).unwrap_or(&[]);

    // Determine if there's a leading escape character we need to handle.
    // skip_esc indicates whether we need to consume an extra byte (the
    // escape character itself) from the buffer.
    let (data, skip_esc) = match data.first() {
        // Double escape: \x1b\x1b or \x1b[
        Some(&b'\x1b') => (&data[1..], true),
        // CSI: \x1b[
        Some(&b'[') => (data, false),
        // SS3: \x1bO
        Some(&b'O') => (data, false),
        // Unknown - return escape key
        _ => {
            if data.first() == Some(&b'\x1b') {
                buf.consume(1);
            }
            return KeyCode::Esc.event();
        }
    };

    // Try each escape sequence format in order of preference
    // CSI-U (Kitty protocol) is tried first as it's the most explicit

    // CSI-U: \x1b[code;modifiersu - Kitty keyboard protocol
    // Supports explicit modifier encoding
    if let Some((consumed, event)) = try_parse_csi_u(data) {
        buf.consume(if skip_esc { 1 + consumed } else { consumed });
        return event;
    }

    // CSI: \x1b[params;modifier@A - Standard CSI sequences
    // Arrow keys, function keys, Home/End with optional modifiers
    if let Some((consumed, event)) = try_parse_csi(data) {
        buf.consume(if skip_esc { 1 + consumed } else { consumed });
        return event;
    }

    // SS3: \x1bOP - Single Shift 3, old function key format
    if let Some((consumed, event)) = try_parse_ss3(data) {
        buf.consume(if skip_esc { 1 + consumed } else { consumed });
        return event;
    }

    // CSI with tilde: \x1b[1~, \x1b[2~, etc.
    // Used for Home, Insert, Delete, Page Up/Down, etc.
    if let Some((consumed, event)) = try_parse_csi_tilde(data) {
        buf.consume(if skip_esc { 1 + consumed } else { consumed });
        return event;
    }

    // Unknown escape sequence - return escape key
    buf.consume(1);
    KeyCode::Esc.event()
}

/// Attempts to parse a CSI-U (Kitty Keyboard Protocol) sequence.
///
/// Format: `\x1b[code;modifiersu`
///
/// The Kitty protocol provides explicit modifier encoding:
/// - Code 1: Esc, 2: Tab, 3: Backspace, 4: Enter
/// - Code 5-8: Home, End, PageUp, PageDown
/// - Code 9-10: Tab (duplicate), Insert
/// - Code 11-22: F1-F12
/// - Code 23: Delete, 24-27: Arrow keys
/// - Code 33-126: Printable characters (ASCII)
///
/// Modifiers: 2=Shift, 3=Alt, 4=Ctrl, 5=Ctrl+Shift, 6=Alt+Ctrl, 7=Alt+Ctrl+Shift
///
/// Returns `Some((bytes_consumed, event))` if successful, `None` otherwise.
pub fn try_parse_csi_u(data: &[u8]) -> Option<(usize, Event)> {
    if !data.starts_with(b"[") {
        return None;
    }

    let params = &data[1..];
    // Must end with 'u' to be a valid CSI-U sequence
    if params.is_empty() || !params.ends_with(b"u") {
        return None;
    }

    let end_pos = params.len() - 1;
    let param_part = &params[..end_pos];

    let param_str = std::str::from_utf8(param_part).ok()?;
    let parts: Vec<&str> = param_str.split(';').collect();
    if parts.is_empty() {
        return None;
    }

    let key_code: u32 = parts[0].parse().ok()?;
    tracing::debug!("CSI-u: code={}, params={}", key_code, param_str);

    // Parse modifiers from second parameter (if present)
    let modifiers = if parts.len() > 1 {
        if let Ok(mod_val) = parts[1].parse::<u8>() {
            Modifiers::from_kitty_encoding(mod_val)
        } else {
            Modifiers::default()
        }
    } else {
        Modifiers::default()
    };

    // Map key code to KeyCode
    let key = match key_code {
        0 => return None, // Invalid key code
        1 => KeyCode::Esc,
        2 => KeyCode::Tab,
        // 3 is not used in CSI-u format (Delete is CSI 3~)
        4 => KeyCode::Enter,
        5 => KeyCode::Home,
        6 => KeyCode::End,
        7 => KeyCode::PageUp,
        8 => KeyCode::PageDown,
        9 => KeyCode::Tab,
        10 => KeyCode::Insert,
        11 => KeyCode::F1,
        12 => KeyCode::F2,
        13 => KeyCode::F3,
        14 => KeyCode::F4,
        15 => KeyCode::F5,
        16 => KeyCode::F6,
        17 => KeyCode::F7,
        18 => KeyCode::F8,
        19 => KeyCode::F9,
        20 => KeyCode::F10,
        21 => KeyCode::F11,
        22 => KeyCode::F12,
        // 23 is not Delete in CSI-u (Delete is CSI 3~, not CSI u)
        24 => KeyCode::Up,
        25 => KeyCode::Down,
        26 => KeyCode::Right,
        27 => KeyCode::Left,
        // 127 is Backspace in Kitty CSI-u format
        127 => KeyCode::Backspace,
        // ASCII printable characters (33-126)
        n @ 33..=126 => KeyCode::Char(n as u8 as char),
        _ => return None,
    };

    Some((params.len() + 1, key.with_modifiers(modifiers).event()))
}

/// Attempts to parse a standard CSI sequence.
///
/// Format: `\x1b[params;modifiers@`
///
/// Final character determines the key:
/// - A, B, C, D: Arrow keys (Up, Down, Right, Left)
/// - H, F: Home, End
/// - P, Q, R, S: F1-F4 (alternate format)
///
/// The modifier byte (before the final character) may contain:
/// - 1: Shift, 2: Alt, 4: Ctrl, 8: Super
///   Combined with 2,4,8 for Ctrl+Shift, etc.
///
/// Returns `Some((bytes_consumed, event))` if successful, `None` otherwise.
pub fn try_parse_csi(data: &[u8]) -> Option<(usize, Event)> {
    if !data.starts_with(b"[") {
        return None;
    }

    let params = &data[1..];
    if params.is_empty() {
        return None;
    }

    // Find the final byte (non-digit, non-semicolon)
    // This marks the end of parameters and start of the key identifier
    let final_byte_pos = params
        .iter()
        .position(|b| !b.is_ascii_digit() && *b != b';')?;
    let final_byte = params.get(final_byte_pos)?;

    // DSR (Device Status Report) response: \x1b[rows;colsR
    // This is a cursor position query response, not a key press
    if *final_byte == b'R' && params.contains(&b';') {
        return try_parse_dsr(data);
    }

    // Map final byte to key code
    let key = match final_byte {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P' => KeyCode::F1,
        b'Q' => KeyCode::F2,
        b'R' => KeyCode::F3,
        b'S' => KeyCode::F4,
        _ => return None,
    };

    let modifiers = parse_modifiers_from_params(params, final_byte_pos);

    let consumed = 1 + final_byte_pos + 1;
    Some((consumed, key.with_modifiers(modifiers).event()))
}

/// Parses modifier flags from CSI parameter bytes.
///
/// The modifier can be encoded in two ways:
/// 1. Kitty protocol: semicolon-separated value (e.g., "1;2" means Shift)
/// 2. Traditional: combined in the parameter (e.g., "1;2" -> modifier value 2)
///
/// This function tries to extract the modifier from the parameter string.
pub fn parse_modifiers_from_params(params: &[u8], final_byte_pos: usize) -> Modifiers {
    let param_str = std::str::from_utf8(&params[..final_byte_pos]).ok();
    if let Some(s) = param_str {
        // Look for a semicolon-separated modifier value
        if let Some(semi_pos) = s.find(';')
            && let Ok(mod_val) = s[semi_pos + 1..].parse::<u8>()
        {
            return Modifiers::from_kitty_encoding(mod_val);
        }
    }
    Modifiers::default()
}

/// Attempts to parse a DSR (Device Status Report) response.
///
/// Format: `\x1b[rows;colsR`
///
/// This is sent by the terminal in response to a cursor position query (DA1).
/// It represents a resize event with the current cursor position, which
/// we interpret as the terminal dimensions.
///
/// Returns `Some((bytes_consumed, event))` where event is `Event::Resize`.
pub fn try_parse_dsr(data: &[u8]) -> Option<(usize, Event)> {
    if !data.starts_with(b"[") {
        return None;
    }

    let params = &data[1..];
    if params.is_empty() {
        return None;
    }

    // Find the 'R' that ends the sequence
    let r_pos = params.iter().position(|&b| b == b'R')?;
    let dims = std::str::from_utf8(&params[..r_pos]).ok()?;

    // Parse rows;cols
    let parts: Vec<&str> = dims.split(';').collect();
    if parts.len() != 2 {
        return None;
    }

    let rows: u16 = parts[0].parse().ok()?;
    let cols: u16 = parts[1].parse().ok()?;

    let consumed = 1 + r_pos + 1;
    Some((consumed, Event::Resize(rows, cols)))
}

/// Attempts to parse an SS3 (Single Shift 3) sequence.
///
/// Format: `\x1bOkey`
///
/// SS3 is an older format used by some terminals for function keys:
/// - OP, OQ, OR, OS: F1-F4
/// - OH, OF: Home, End
/// - OV, OW: PageUp, PageDown
///
/// Note: Insert and Delete are NOT defined in SS3 format per Kitty spec.
/// They should be sent via CSI-tilde sequences (`\x1b[2~`, `\x1b[3~`).
///
/// Returns `Some((bytes_consumed, event))` if successful, `None` otherwise.
pub fn try_parse_ss3(data: &[u8]) -> Option<(usize, Event)> {
    if !data.starts_with(b"O") || data.len() < 2 {
        return None;
    }

    let key = match data[1] {
        b'P' => KeyCode::F1,
        b'Q' => KeyCode::F2,
        b'R' => KeyCode::F3,
        b'S' => KeyCode::F4,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'V' => KeyCode::PageUp,
        b'W' => KeyCode::PageDown,
        // Insert and Delete are NOT standard SS3 codes per Kitty protocol
        // They should use CSI-tilde sequences: \x1b[2~ for Insert, \x1b[3~ for Delete
        _ => return None,
    };

    Some((2, key.event()))
}

/// Attempts to parse a CSI sequence with tilde suffix.
///
/// Format: `\x1b[num~` or `\x1b[num;mod~`
///
/// Used for various special keys:
/// - 1~, 7~: Home
/// - 2~: Insert
/// - 3~: Delete
/// - 4~, 8~: End
/// - 5~, 6~: PageUp, PageDown
/// - 11-14: F1-F4
/// - 15-21: F5-F9, F11
/// - 23, 24: F12
///
/// Returns `Some((bytes_consumed, event))` if successful, `None` otherwise.
pub fn try_parse_csi_tilde(data: &[u8]) -> Option<(usize, Event)> {
    if !data.starts_with(b"[") {
        return None;
    }

    let params = &data[1..];
    if params.is_empty() {
        return None;
    }

    // Find the tilde (~) that marks the end of the sequence
    let tilde_pos = params.iter().position(|&b| b == b'~')?;
    let num_str = std::str::from_utf8(&params[..tilde_pos]).ok()?;
    let parts: Vec<&str> = num_str.split(';').collect();
    let num: u32 = parts[0].parse().ok()?;

    // Map number to key code
    let key = match num {
        1 | 7 => KeyCode::Home,
        2 => KeyCode::Insert,
        3 => KeyCode::Delete,
        4 | 8 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        11 => KeyCode::F1,
        12 => KeyCode::F2,
        13 => KeyCode::F3,
        14 => KeyCode::F4,
        15 => KeyCode::F5,
        16 => KeyCode::F6,
        17 => KeyCode::F6, // Some terminals duplicate F6
        18 => KeyCode::F7,
        19 => KeyCode::F8,
        20 => KeyCode::F9,
        21 => KeyCode::F11,
        23 => KeyCode::F12,
        24 => KeyCode::F12,
        _ => return None,
    };

    // Parse optional modifiers from second parameter
    let modifiers = if parts.len() > 1 {
        if let Ok(mod_val) = parts[1].parse::<u8>() {
            Modifiers::from_kitty_encoding(mod_val)
        } else {
            Modifiers::default()
        }
    } else {
        Modifiers::default()
    };

    let consumed = tilde_pos + 2;
    Some((consumed, key.with_modifiers(modifiers).event()))
}
