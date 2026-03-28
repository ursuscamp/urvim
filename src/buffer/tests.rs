use super::*;
use crate::buffer::operator_target::LinewiseDeleteRange;
use crate::editor::{
    BoundaryMotion, BracketKind, LinewiseMotion, OperatorTarget, QuoteKind, TextObject,
};
use crate::path::AbsolutePath;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "urvim-buffer-tests-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        name
    ))
}

#[test]
fn test_new_buffer() {
    let buf = Buffer::new();
    assert!(buf.is_empty());
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), "");
    assert!(!buf.is_modified());
}

#[test]
fn test_from_str() {
    let buf = Buffer::from_str("hello");
    assert!(!buf.is_empty());
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), "hello");
    assert!(!buf.is_modified());
}

#[test]
fn test_filetype_from_shebang() {
    let buf = Buffer::from_str("#!/usr/bin/env python3 -O\nprint('hello')");
    assert_eq!(buf.filetype(), Filetype::Python);
}

#[test]
fn test_filetype_from_filename() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.php")).unwrap();
    let buf = Buffer::from_str_with_path("<?php echo 'hello';", path);

    assert_eq!(buf.filetype(), Filetype::Php);
}

#[test]
fn test_filetype_filename_takes_precedence_over_shebang() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.rs")).unwrap();
    let buf = Buffer::from_str_with_path("#!/usr/bin/env python3\nprint('hello')", path);

    assert_eq!(buf.filetype(), Filetype::Rust);
}

#[test]
fn test_filetype_updates_after_first_line_edit() {
    let mut buf = Buffer::from_str("#!/usr/bin/env python3\nprint('hello')");

    assert_eq!(buf.filetype(), Filetype::Python);

    let shebang_len = buf.line_len(0);
    buf.remove(Cursor::new(0, 0), Cursor::new(0, shebang_len));
    buf.insert_text(Cursor::new(0, 0), "plain text");

    assert_eq!(buf.filetype(), Filetype::Python);
    buf.mark_saved();
    assert_eq!(buf.filetype(), Filetype::PlainText);
}

#[test]
fn test_modified_state_tracks_edits_and_undo() {
    let mut buf = Buffer::from_str("hello");

    assert!(!buf.is_modified());
    buf.insert_char(Cursor::new(0, 5), '!');
    assert!(buf.is_modified());
    buf.mark_saved();
    assert!(!buf.is_modified());
}

#[test]
fn test_from_str_multiline() {
    let buf = Buffer::from_str("hello\nworld");
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "hello\nworld");
}

#[test]
fn test_from_str_trailing_newline() {
    let buf = Buffer::from_str("hello\n");
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), "hello");
}

#[test]
fn test_insert_char() {
    let mut buf = Buffer::from_str("hello");
    buf.insert_char(Cursor::new(0, 5), '!');
    assert_eq!(buf.as_str(), "hello!");
}

#[test]
fn test_insert_text() {
    let mut buf = Buffer::from_str("hello");
    buf.insert_text(Cursor::new(0, 5), " world");
    assert_eq!(buf.as_str(), "hello world");
}

#[test]
fn test_insert_at_beginning() {
    let mut buf = Buffer::from_str("world");
    buf.insert_text(Cursor::new(0, 0), "hello ");
    assert_eq!(buf.as_str(), "hello world");
}

#[test]
fn test_insert_in_middle() {
    let mut buf = Buffer::from_str("hello");
    buf.insert_text(Cursor::new(0, 2), "XX");
    assert_eq!(buf.as_str(), "heXXllo");
}

#[test]
fn test_insert_with_newline() {
    let mut buf = Buffer::from_str("hello");
    buf.insert_text(Cursor::new(0, 2), "X\nY");
    assert_eq!(buf.as_str(), "heX\nYllo");
    assert_eq!(buf.line_count(), 2);
}

#[test]
fn test_remove() {
    let mut buf = Buffer::from_str("hello world");
    buf.remove(Cursor::new(0, 5), Cursor::new(0, 11));
    assert_eq!(buf.as_str(), "hello");
}

#[test]
fn test_remove_from_beginning() {
    let mut buf = Buffer::from_str("hello");
    buf.remove(Cursor::new(0, 0), Cursor::new(0, 2));
    assert_eq!(buf.as_str(), "llo");
}

#[test]
fn test_remove_multiline() {
    let mut buf = Buffer::from_str("hello\nworld");
    buf.remove(Cursor::new(0, 2), Cursor::new(1, 2));
    assert_eq!(buf.as_str(), "herld");
}

#[test]
fn test_line_count() {
    let buf = Buffer::from_str("line1\nline2\nline3");
    assert_eq!(buf.line_count(), 3);
}

#[test]
fn test_line_count_single_line() {
    let buf = Buffer::from_str("hello");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_line_count_empty() {
    let buf = Buffer::new();
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_line_at() {
    let buf = Buffer::from_str("line1\nline2\nline3");
    assert_eq!(buf.line_at(0).map(|s| s.as_ref() as &str), Some("line1"));
    assert_eq!(buf.line_at(1).map(|s| s.as_ref() as &str), Some("line2"));
    assert_eq!(buf.line_at(2).map(|s| s.as_ref() as &str), Some("line3"));
}

#[test]
fn test_line_at_out_of_bounds() {
    let buf = Buffer::from_str("hello");
    assert!(buf.line_at(1).is_none());
}

#[test]
fn test_line_grapheme_len() {
    let buf = Buffer::from_str("a😀c\n");
    assert_eq!(buf.line_at(0).map(|s| str_width(s.as_ref())), Some(4));
}

#[test]
fn test_save_and_load() {
    let path = temp_path("save_and_load.txt");
    let buf = Buffer::from_str("hello world");
    buf.save_to_file(&path).unwrap();

    let loaded = Buffer::load_from_file(&path).unwrap();
    assert_eq!(loaded.as_str(), "hello world");

    fs::remove_file(&path).ok();
}

#[test]
fn test_save_buffer_clears_modified_state_and_refreshes_filetype() {
    let path = temp_path("save_buffer");
    fs::write(&path, "#!/usr/bin/env python3\nprint('hello')").unwrap();

    let mut pool = BufferPool::new();
    let id = pool.open_buffer(&path).unwrap();

    pool.with_buffer_mut(id, |buffer| {
        let shebang_len = buffer.line_len(0);
        buffer.remove(Cursor::new(0, 0), Cursor::new(0, shebang_len));
        buffer.insert_text(Cursor::new(0, 0), "plain text");
    })
    .unwrap();

    assert!(pool.get(id).unwrap().is_modified());
    assert_eq!(pool.get(id).unwrap().filetype(), Filetype::Python);

    pool.save_buffer(id).unwrap();

    assert!(!pool.get(id).unwrap().is_modified());
    assert_eq!(pool.get(id).unwrap().filetype(), Filetype::PlainText);

    fs::remove_file(&path).ok();
}

#[test]
fn test_save_buffer_without_path_is_rejected() {
    let mut pool = BufferPool::new();
    let id = pool.register_buffer(Buffer::from_str("hello"));

    let err = pool
        .save_buffer(id)
        .expect_err("unnamed buffers should not save");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn test_multiline_with_empty_lines() {
    let buf = Buffer::from_str("a\n\nb");
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.line_at(0).map(|s| s.as_ref() as &str), Some("a"));
    assert_eq!(buf.line_at(1).map(|s| s.as_ref() as &str), Some(""));
    assert_eq!(buf.line_at(2).map(|s| s.as_ref() as &str), Some("b"));
}

#[test]
fn test_remove_all() {
    let mut buf = Buffer::from_str("hello");
    buf.remove(Cursor::new(0, 0), Cursor::new(0, 5));
    assert!(buf.is_empty());
    assert_eq!(buf.as_str(), "");
}

#[test]
fn test_insert_into_empty() {
    let mut buf = Buffer::new();
    buf.insert_text(Cursor::new(0, 0), "test");
    assert_eq!(buf.as_str(), "test");
}

#[test]
fn test_line_with_tab() {
    let buf = Buffer::from_str("a\tb");
    assert_eq!(buf.line_at(0).map(|s| s.len()), Some(3));
}

#[test]
fn test_char_width_ascii() {
    assert_eq!(char_width('a'), 1);
    assert_eq!(char_width('z'), 1);
}

#[test]
fn test_char_width_cjk() {
    assert_eq!(char_width('中'), 2);
    assert_eq!(char_width('日'), 2);
}

#[test]
fn test_char_width_narrow() {
    assert_eq!(char_width('\t'), 0);
}

#[test]
fn test_str_width() {
    assert_eq!(str_width("hello"), 5);
    assert_eq!(str_width("helło"), 5);
    assert_eq!(str_width("你好"), 4);
    assert_eq!(str_width("😀"), 2);
}

#[test]
fn test_grapheme_width() {
    assert_eq!(grapheme_width("a"), 1);
    assert_eq!(grapheme_width("😀"), 2);
    assert_eq!(grapheme_width("中"), 2);
}

#[test]
fn test_visual_col_at() {
    let buf = Buffer::from_str("a😀c");
    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 3);
}

#[test]
fn test_buffer_len() {
    let buf = Buffer::from_str("abc\ndef");
    assert_eq!(buf.len(), 7); // 3 + 1 + 3
}

// Cursor tests

#[test]
fn test_cursor_new() {
    let cursor = Cursor::new(0, 0);
    assert_eq!(cursor.line, 0);
    assert_eq!(cursor.col, 0);
}

#[test]
fn test_cursor_default() {
    let cursor = Cursor::default();
    assert_eq!(cursor, Cursor::new(0, 0));
}

#[test]
fn test_cursor_partial_eq() {
    let c1 = Cursor::new(0, 5);
    let c2 = Cursor::new(0, 5);
    let c3 = Cursor::new(1, 5);
    assert_eq!(c1, c2);
    assert_ne!(c1, c3);
}

#[test]
fn test_is_valid_cursor() {
    let buf = Buffer::from_str("hello");
    assert!(buf.is_valid_cursor(Cursor::new(0, 0)));
    assert!(buf.is_valid_cursor(Cursor::new(0, 3)));
    assert!(buf.is_valid_cursor(Cursor::new(0, 5))); // at end
    assert!(!buf.is_valid_cursor(Cursor::new(0, 6))); // beyond line
    assert!(!buf.is_valid_cursor(Cursor::new(1, 0))); // beyond last line
}

#[test]
fn test_is_valid_cursor_multiline() {
    let buf = Buffer::from_str("hello\nworld");
    assert!(buf.is_valid_cursor(Cursor::new(0, 0)));
    assert!(buf.is_valid_cursor(Cursor::new(0, 5)));
    assert!(buf.is_valid_cursor(Cursor::new(1, 0)));
    assert!(buf.is_valid_cursor(Cursor::new(1, 5)));
    assert!(!buf.is_valid_cursor(Cursor::new(1, 6)));
    assert!(!buf.is_valid_cursor(Cursor::new(2, 0)));
}

// next_cursor tests

#[test]
fn test_next_cursor_ascii() {
    let buf = Buffer::from_str("hello");

    assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
    assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 2)));
    assert_eq!(buf.next_cursor(Cursor::new(0, 4)), Some(Cursor::new(0, 5)));
    assert_eq!(buf.next_cursor(Cursor::new(0, 5)), None); // at end of line, last line
}

#[test]
fn test_next_cursor_multibyte() {
    let buf = Buffer::from_str("aβc"); // 'β' is 2 bytes

    assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
    assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 3))); // jump over β
    assert_eq!(buf.next_cursor(Cursor::new(0, 3)), Some(Cursor::new(0, 4)));
}

#[test]
fn test_next_cursor_emoji() {
    let buf = Buffer::from_str("a😀c"); // emoji is 4 bytes

    assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
    assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 5))); // jump over emoji
    assert_eq!(buf.next_cursor(Cursor::new(0, 5)), Some(Cursor::new(0, 6)));
}

#[test]
fn test_next_cursor_across_newline() {
    let buf = Buffer::from_str("ab\ncd");

    // "ab" has byte len 2, "cd" has byte len 2
    assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
    assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 2)));
    assert_eq!(buf.next_cursor(Cursor::new(0, 2)), Some(Cursor::new(1, 0))); // cross newline
    assert_eq!(buf.next_cursor(Cursor::new(1, 0)), Some(Cursor::new(1, 1)));
    // At col 2 (end of "cd"), moving right goes past end -> None
    assert_eq!(buf.next_cursor(Cursor::new(1, 2)), None);
}

#[test]
fn test_next_cursor_at_end_of_last_line() {
    let buf = Buffer::from_str("ab\ncd");

    // At end of last line, moving right stays in place (returns None)
    assert_eq!(buf.next_cursor(Cursor::new(1, 2)), None);
}

// prev_cursor tests

#[test]
fn test_prev_cursor_ascii() {
    let buf = Buffer::from_str("hello");

    assert_eq!(buf.prev_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
    assert_eq!(buf.prev_cursor(Cursor::new(0, 5)), Some(Cursor::new(0, 4)));
    assert_eq!(buf.prev_cursor(Cursor::new(0, 0)), None); // at start
}

#[test]
fn test_prev_cursor_multibyte() {
    let buf = Buffer::from_str("aβc"); // 'β' is 2 bytes

    assert_eq!(buf.prev_cursor(Cursor::new(0, 3)), Some(Cursor::new(0, 1))); // jump over β
    assert_eq!(buf.prev_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
}

#[test]
fn test_prev_cursor_emoji() {
    let buf = Buffer::from_str("a😀c"); // emoji is 4 bytes

    assert_eq!(buf.prev_cursor(Cursor::new(0, 5)), Some(Cursor::new(0, 1))); // jump over emoji
    assert_eq!(buf.prev_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
}

#[test]
fn test_prev_cursor_across_newline() {
    let buf = Buffer::from_str("ab\ncd");

    assert_eq!(buf.prev_cursor(Cursor::new(1, 0)), Some(Cursor::new(0, 2))); // cross newline
    assert_eq!(buf.prev_cursor(Cursor::new(0, 2)), Some(Cursor::new(0, 1)));
}

#[test]
fn test_prev_cursor_at_start() {
    let buf = Buffer::from_str("ab");

    assert_eq!(buf.prev_cursor(Cursor::new(0, 0)), None);
}

// next_cursor_line tests

#[test]
fn test_next_cursor_line_ascii() {
    let buf = Buffer::from_str("hello");

    // At start, next is col 1
    assert_eq!(
        buf.next_cursor_line(Cursor::new(0, 0)),
        Some(Cursor::new(0, 1))
    );
    // At middle
    assert_eq!(
        buf.next_cursor_line(Cursor::new(0, 2)),
        Some(Cursor::new(0, 3))
    );
    // At last char, next is end of line
    assert_eq!(
        buf.next_cursor_line(Cursor::new(0, 4)),
        Some(Cursor::new(0, 5))
    );
    // At end of line, None
    assert_eq!(buf.next_cursor_line(Cursor::new(0, 5)), None);
}

#[test]
fn test_next_cursor_line_emoji() {
    let buf = Buffer::from_str("a😀b"); // emoji is 4 bytes

    // At 'a', next is start of emoji
    assert_eq!(
        buf.next_cursor_line(Cursor::new(0, 0)),
        Some(Cursor::new(0, 1))
    );
    // At emoji, next is 'b'
    assert_eq!(
        buf.next_cursor_line(Cursor::new(0, 1)),
        Some(Cursor::new(0, 5))
    );
    // At 'b', next is end of line
    assert_eq!(
        buf.next_cursor_line(Cursor::new(0, 5)),
        Some(Cursor::new(0, 6))
    );
    // At end of line, None
    assert_eq!(buf.next_cursor_line(Cursor::new(0, 6)), None);
}

#[test]
fn test_next_cursor_line_at_end_of_line() {
    let buf = Buffer::from_str("hello");

    // At end of line, returns None (doesn't wrap to next line)
    assert_eq!(buf.next_cursor_line(Cursor::new(0, 5)), None);
}

// prev_cursor_line tests

#[test]
fn test_prev_cursor_line_ascii() {
    let buf = Buffer::from_str("hello");

    // At col 1, prev is col 0
    assert_eq!(
        buf.prev_cursor_line(Cursor::new(0, 1)),
        Some(Cursor::new(0, 0))
    );
    // At col 3, prev is col 2
    assert_eq!(
        buf.prev_cursor_line(Cursor::new(0, 3)),
        Some(Cursor::new(0, 2))
    );
    // At start, None
    assert_eq!(buf.prev_cursor_line(Cursor::new(0, 0)), None);
}

#[test]
fn test_prev_cursor_line_emoji() {
    let buf = Buffer::from_str("a😀b"); // emoji is 4 bytes

    // At emoji start, prev is 'a'
    assert_eq!(
        buf.prev_cursor_line(Cursor::new(0, 1)),
        Some(Cursor::new(0, 0))
    );
    // At 'b', prev is emoji start
    assert_eq!(
        buf.prev_cursor_line(Cursor::new(0, 5)),
        Some(Cursor::new(0, 1))
    );
    // At start, None
    assert_eq!(buf.prev_cursor_line(Cursor::new(0, 0)), None);
}

#[test]
fn test_prev_cursor_line_at_start_of_line() {
    let buf = Buffer::from_str("hello");

    // At start of line, returns None (doesn't wrap to prev line)
    assert_eq!(buf.prev_cursor_line(Cursor::new(0, 0)), None);
}

// cursor_down tests

#[test]
fn test_cursor_down_preserves_visual_col() {
    let buf = Buffer::from_str("ab\ncd");

    assert_eq!(
        buf.cursor_down(Cursor::new(0, 0), 0),
        Some(Cursor::new(1, 0))
    );
    assert_eq!(
        buf.cursor_down(Cursor::new(0, 1), 1),
        Some(Cursor::new(1, 1))
    );
    assert_eq!(
        buf.cursor_down(Cursor::new(0, 2), 2),
        Some(Cursor::new(1, 2))
    );
}

#[test]
fn test_cursor_down_with_emoji() {
    let buf = Buffer::from_str("a😀\nb");

    // a😀 has visual width 3 (1 + 2), b has visual width 1
    // visual col 1 should map to byte 1 (after 'a')
    assert_eq!(
        buf.cursor_down(Cursor::new(0, 0), 0),
        Some(Cursor::new(1, 0))
    );
    assert_eq!(
        buf.cursor_down(Cursor::new(0, 1), 1),
        Some(Cursor::new(1, 1))
    ); // after 'a'
    // visual col 2 would be in middle of emoji, should clamp to end of next line
    assert_eq!(
        buf.cursor_down(Cursor::new(0, 5), 3),
        Some(Cursor::new(1, 1))
    ); // end of "b"
}

#[test]
fn test_cursor_down_short_line_clamps() {
    let buf = Buffer::from_str("ab\nc");

    // Line 0 has "ab" (2 chars), Line 1 has "c" (1 char)
    // From col 2 on line 0, going down should clamp to col 1 (end of line 1)
    assert_eq!(
        buf.cursor_down(Cursor::new(0, 2), 2),
        Some(Cursor::new(1, 1))
    );
}

#[test]
fn test_cursor_down_at_last_line() {
    let buf = Buffer::from_str("ab\ncd");

    // At last line, should return None
    assert_eq!(buf.cursor_down(Cursor::new(1, 0), 0), None);
}

// cursor_up tests

#[test]
fn test_cursor_up_preserves_visual_col() {
    let buf = Buffer::from_str("ab\ncd");

    assert_eq!(buf.cursor_up(Cursor::new(1, 0), 0), Some(Cursor::new(0, 0)));
    assert_eq!(buf.cursor_up(Cursor::new(1, 1), 1), Some(Cursor::new(0, 1)));
    assert_eq!(buf.cursor_up(Cursor::new(1, 2), 2), Some(Cursor::new(0, 2)));
}

#[test]
fn test_cursor_up_with_emoji() {
    let buf = Buffer::from_str("a\nb😀");

    // Going up from line 1 should preserve visual column
    assert_eq!(buf.cursor_up(Cursor::new(1, 0), 0), Some(Cursor::new(0, 0)));
    assert_eq!(buf.cursor_up(Cursor::new(1, 1), 1), Some(Cursor::new(0, 1)));
}

#[test]
fn test_cursor_up_short_line_clamps() {
    let buf = Buffer::from_str("ab\nc");

    // Line 0 has "ab" (2 chars), Line 1 has "c" (1 char)
    // From col 1 on line 1, going up should stay at col 1
    assert_eq!(buf.cursor_up(Cursor::new(1, 1), 1), Some(Cursor::new(0, 1)));
}

#[test]
fn test_cursor_up_at_first_line() {
    let buf = Buffer::from_str("ab\ncd");

    // At first line, should return None
    assert_eq!(buf.cursor_up(Cursor::new(0, 0), 0), None);
}

// cursor_end_of_line tests

#[test]
fn test_cursor_end_of_line_middle_of_line() {
    let buf = Buffer::from_str("hello");

    // In middle of line, move to end
    assert_eq!(
        buf.cursor_end_of_line(Cursor::new(0, 2)),
        Some(Cursor::new(0, 4))
    );
}

#[test]
fn test_cursor_end_of_line_at_end_wraps() {
    let buf = Buffer::from_str("hello\nworld");

    // At end of line, wraps to next line's end
    assert_eq!(
        buf.cursor_end_of_line(Cursor::new(0, 4)),
        Some(Cursor::new(1, 4))
    );
}

#[test]
fn test_cursor_end_of_line_at_end_of_last_line() {
    let buf = Buffer::from_str("hello\nworld");

    // At end of last line, no movement
    assert_eq!(buf.cursor_end_of_line(Cursor::new(1, 4)), None);
}

#[test]
fn test_cursor_end_of_line_empty_buffer() {
    let buf = Buffer::new();

    // Empty buffer, no movement
    assert_eq!(buf.cursor_end_of_line(Cursor::new(0, 0)), None);
}

#[test]
fn test_cursor_end_of_line_empty_line() {
    let buf = Buffer::from_str("hello\n\nworld");

    // Empty line in middle, wrap to next line (empty line)
    assert_eq!(
        buf.cursor_end_of_line(Cursor::new(0, 4)),
        Some(Cursor::new(1, 0))
    );
}

#[test]
fn test_cursor_end_of_line_with_trailing_whitespace() {
    let buf = Buffer::from_str("hello   ");

    // Should move to last non-whitespace character
    assert_eq!(
        buf.cursor_end_of_line(Cursor::new(0, 0)),
        Some(Cursor::new(0, 4))
    );
}

#[test]
fn test_cursor_end_of_line_with_wide_characters() {
    let buf = Buffer::from_str("hello😀world");

    // "hello" (5 bytes) + "😀" (4 bytes) = 9 bytes, then "world" (5 bytes) = 14 bytes total
    // Last char 'd' is at byte 13
    assert_eq!(
        buf.cursor_end_of_line(Cursor::new(0, 0)),
        Some(Cursor::new(0, 13))
    );
}

// cursor_start_of_line tests

#[test]
fn test_cursor_start_of_line_middle_of_line() {
    let buf = Buffer::from_str("  hello");

    // In middle of line - move to column 0
    assert_eq!(
        buf.cursor_start_of_line(Cursor::new(0, 5)),
        Some(Cursor::new(0, 0))
    );
}

#[test]
fn test_cursor_start_of_line_at_column_zero_wraps() {
    let buf = Buffer::from_str("  hello\n  world");

    // At column 0 on line 1 - wrap to previous line
    assert_eq!(
        buf.cursor_start_of_line(Cursor::new(1, 0)),
        Some(Cursor::new(0, 0))
    );
}

#[test]
fn test_cursor_start_of_line_at_first_line_no_wrap() {
    let buf = Buffer::from_str("  hello");

    // At column 0 on first line - no movement
    assert_eq!(buf.cursor_start_of_line(Cursor::new(0, 0)), None);
}

#[test]
fn test_cursor_start_of_line_empty_buffer() {
    let buf = Buffer::from_str("");

    // Empty buffer - no movement
    assert_eq!(buf.cursor_start_of_line(Cursor::new(0, 0)), None);
}

// cursor_content_start_of_line tests

#[test]
fn test_cursor_content_start_of_line_middle_of_line() {
    let buf = Buffer::from_str("  hello");

    // In middle of line - move to first non-whitespace
    assert_eq!(
        buf.cursor_content_start_of_line(Cursor::new(0, 5)),
        Some(Cursor::new(0, 2))
    );
}

#[test]
fn test_cursor_content_start_of_line_at_first_non_ws() {
    let buf = Buffer::from_str("  hello\n  world");

    // At first non-whitespace on line 1 - wrap to previous line (line 0)
    assert_eq!(
        buf.cursor_content_start_of_line(Cursor::new(1, 2)),
        Some(Cursor::new(0, 2))
    );
}

#[test]
fn test_cursor_content_start_of_line_at_first_line_no_wrap() {
    let buf = Buffer::from_str("  hello");

    // At first non-whitespace of first line - no movement
    assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 2)), None);
}

#[test]
fn test_cursor_content_start_of_line_no_leading_whitespace() {
    let buf = Buffer::from_str("hello");

    // No leading whitespace - already at first non-whitespace
    assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 0)), None);
}

#[test]
fn test_cursor_content_start_of_line_empty_buffer() {
    let buf = Buffer::from_str("");

    // Empty buffer - no movement
    assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 0)), None);
}

#[test]
fn test_cursor_content_start_of_line_empty_line() {
    let buf = Buffer::from_str("  \nhello");

    // At first non-whitespace on line 1 - wrap to previous line which is empty
    // Previous line has no non-whitespace, so move to column 0
    assert_eq!(
        buf.cursor_content_start_of_line(Cursor::new(1, 0)),
        Some(Cursor::new(0, 0))
    );
}

#[test]
fn test_cursor_content_start_of_line_with_wide_characters() {
    let buf = Buffer::from_str("  hello😀world");

    // First non-whitespace is 'h' at byte 2
    assert_eq!(
        buf.cursor_content_start_of_line(Cursor::new(0, 5)),
        Some(Cursor::new(0, 2))
    );
}

// visual_col_at tests

#[test]
fn test_visual_col_at_cursor() {
    let buf = Buffer::from_str("a😀c");

    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 3);
}

#[test]
fn test_visual_col_at_multiline() {
    let buf = Buffer::from_str("ab\ncd");

    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 2)), 2);
    assert_eq!(buf.visual_col_at(Cursor::new(1, 0)), 0);
}

// byte_pos_at_visual_col tests

#[test]
fn test_byte_pos_at_visual_col() {
    let buf = Buffer::from_str("a😀c");

    assert_eq!(buf.byte_pos_at_visual_col(0, 0), 0);
    assert_eq!(buf.byte_pos_at_visual_col(0, 1), 1);
    assert_eq!(buf.byte_pos_at_visual_col(0, 2), 1); // middle of emoji
    assert_eq!(buf.byte_pos_at_visual_col(0, 3), 5);
    assert_eq!(buf.byte_pos_at_visual_col(0, 10), 6); // beyond line
}

// line_len tests

#[test]
fn test_line_len() {
    let buf = Buffer::from_str("hello\nworld");

    assert_eq!(buf.line_len(0), 5);
    assert_eq!(buf.line_len(1), 5);
}

#[test]
fn test_line_len_out_of_bounds() {
    let buf = Buffer::from_str("hello");

    assert_eq!(buf.line_len(1), 0);
}

#[test]
fn test_insert_char_ascii_cursor_mapping() {
    let mut buf = Buffer::new();
    let cursor = Cursor::new(0, 0);

    buf.insert_char(cursor, 'h');
    buf.insert_char(Cursor::new(0, 1), 'e');
    buf.insert_char(Cursor::new(0, 2), 'l');
    buf.insert_char(Cursor::new(0, 3), 'l');
    buf.insert_char(Cursor::new(0, 4), 'o');

    assert_eq!(buf.as_str(), "hello");
    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 2)), 2);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 3)), 3);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 4)), 4);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 5);
}

#[test]
fn test_insert_char_wide_cursor_mapping() {
    let mut buf = Buffer::new();
    let cursor = Cursor::new(0, 0);

    buf.insert_char(cursor, '日');
    buf.insert_char(Cursor::new(0, 3), '本');
    buf.insert_char(Cursor::new(0, 6), '語');

    assert_eq!(buf.as_str(), "日本語");
    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 3)), 2);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 6)), 4);
}

#[test]
fn test_insert_char_emoji_cursor_mapping() {
    let mut buf = Buffer::new();
    let cursor = Cursor::new(0, 0);

    buf.insert_char(cursor, 'a');
    buf.insert_char(Cursor::new(0, 1), '😀');
    buf.insert_char(Cursor::new(0, 5), 'b');

    assert_eq!(buf.as_str(), "a😀b");
    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 3);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 6)), 4);
}

#[test]
fn test_insert_newline_cursor_mapping() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 5);

    buf.insert_char(cursor, '\n');

    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "hello\n");
    assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 5);
    assert_eq!(buf.visual_col_at(Cursor::new(1, 0)), 0);
}

#[test]
fn test_insert_newline_mid_line_cursor_mapping() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 2);

    buf.insert_char(cursor, '\n');

    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "he\nllo");
    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 2)), 2);
    assert_eq!(buf.visual_col_at(Cursor::new(1, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(1, 3)), 3);
}

#[test]
fn test_insert_mixed_ascii_wide_cursor_mapping() {
    let mut buf = Buffer::new();

    buf.insert_char(Cursor::new(0, 0), 'a');
    buf.insert_char(Cursor::new(0, 1), '日');
    buf.insert_char(Cursor::new(0, 4), 'b');
    buf.insert_char(Cursor::new(0, 5), '本');
    buf.insert_char(Cursor::new(0, 8), 'c');

    assert_eq!(buf.as_str(), "a日b本c");
    assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 4)), 3);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 4);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 8)), 6);
    assert_eq!(buf.visual_col_at(Cursor::new(0, 9)), 7);
}

#[test]
fn test_insert_between_wide_chars_via_cursor_movement() {
    let mut buf = Buffer::from_str("日本語");

    assert_eq!(buf.as_str(), "日本語");
    assert_eq!(buf.line_len(0), 9);

    let cursor_after_first_char = buf.next_cursor(Cursor::new(0, 0));
    assert_eq!(cursor_after_first_char, Some(Cursor::new(0, 3)));

    if let Some(cursor) = cursor_after_first_char {
        buf.insert_char(cursor, 'X');
    }

    assert_eq!(buf.as_str(), "日X本語");

    let cursor_after_insert = buf.next_cursor(cursor_after_first_char.unwrap());
    assert_eq!(cursor_after_insert, Some(Cursor::new(0, 4)));
}

#[test]
fn test_insert_between_emoji_via_cursor_movement() {
    let mut buf = Buffer::from_str("😀😀");

    assert_eq!(buf.as_str(), "😀😀");
    assert_eq!(buf.line_len(0), 8);

    let cursor_after_first_emoji = buf.next_cursor(Cursor::new(0, 0));
    assert_eq!(cursor_after_first_emoji, Some(Cursor::new(0, 4)));

    if let Some(cursor) = cursor_after_first_emoji {
        buf.insert_char(cursor, 'X');
    }

    assert_eq!(buf.as_str(), "😀X😀");

    let cursor_after_insert = buf.next_cursor(cursor_after_first_emoji.unwrap());
    assert_eq!(cursor_after_insert, Some(Cursor::new(0, 5)));
}

#[test]
fn test_insert_mid_emoji_via_cursor_movement() {
    let mut buf = Buffer::from_str("a😀b");

    assert_eq!(buf.as_str(), "a😀b");

    let cursor_after_emoji = buf.next_cursor(Cursor::new(0, 1));
    assert_eq!(cursor_after_emoji, Some(Cursor::new(0, 5)));

    if let Some(cursor) = cursor_after_emoji {
        buf.insert_char(cursor, 'X');
    }

    assert_eq!(buf.as_str(), "a😀Xb");
}

// Boundary motion tests

#[test]
fn test_word_forward_at_last_word() {
    // At position 10 ('d'), w should go to... wait, there's no next line, so should wrap or stay
    // Actually "hello world\nmore" - at 'd' in "world", w should go to 'm' in "more"
    let buf2 = Buffer::from_str("hello world\nmore");
    let result = buf2.next_boundary(Cursor::new(0, 10), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(1, 0))); // wraps to 'm' on line 1
}

#[test]
fn test_word_forward_wrap_no_leading_whitespace() {
    // "hello\nworld" - at 'd' in "hello", w should go to 'w' on line 1 (first word)
    let buf = Buffer::from_str("hello\nworld");
    let result = buf.next_boundary(Cursor::new(0, 4), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(1, 0))); // wraps to 'w' on line 1
}

#[test]
fn test_word_forward_at_word_end() {
    // "hello world" - at 'h' (start of "hello"), w should go to 'w' (start of "world")
    // This is NOT a wrapping case - it's moving to the next word on the same line
    let buf = Buffer::from_str("hello world");
    let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 6))); // 'w'
}

#[test]
fn test_word_forward_at_word_end_with_nonword() {
    // "hello---world" - at 'o' (position 4, end of "hello")
    // w should go to position 5 (first "-")
    // e should go to position 7 (end of "---")
    let buf = Buffer::from_str("hello---world");

    let result_w = buf.next_boundary(Cursor::new(0, 4), Boundary::Word);
    assert_eq!(
        result_w,
        Some(Cursor::new(0, 5)),
        "w should go to first '-'"
    );

    let result_e = buf.next_boundary(Cursor::new(0, 4), Boundary::WordEnd);
    assert_eq!(result_e, Some(Cursor::new(0, 7)), "e should go to last '-'");
}

#[test]
fn test_word_end_at_nonword_sequence_end() {
    // "hello---world" - at last '-' (position 7), e should go to end of "world" (position 12)
    let buf = Buffer::from_str("hello---world");

    let result = buf.next_boundary(Cursor::new(0, 7), Boundary::WordEnd);
    assert_eq!(
        result,
        Some(Cursor::new(0, 12)),
        "e should go to end of 'world'"
    );
}

#[test]
fn test_word_end_at_word_start() {
    // "hello world" - at 'h', e should go to 'o' (end of "hello")
    let buf = Buffer::from_str("hello world");
    let result = buf.next_boundary(Cursor::new(0, 0), Boundary::WordEnd);
    assert_eq!(result, Some(Cursor::new(0, 4))); // 'o'
}

#[test]
fn test_word_end_at_word_end() {
    // "hello world" - at 'o' (end of "hello"), e should go to 'd' (end of "world")
    let buf = Buffer::from_str("hello world");
    let result = buf.next_boundary(Cursor::new(0, 4), Boundary::WordEnd);
    assert_eq!(result, Some(Cursor::new(0, 10))); // 'd'
}

#[test]
fn test_word_end_at_last_char_wraps() {
    // "hello world\nfoo" - at 'd' in "world", e should wrap to 'o' in "foo"
    let buf = Buffer::from_str("hello world\nfoo");
    let result = buf.next_boundary(Cursor::new(0, 10), Boundary::WordEnd);
    assert_eq!(result, Some(Cursor::new(1, 2))); // 'o' in "foo"
}

#[test]
fn test_bigword_forward_wrap_no_leading_whitespace() {
    // "hello\nworld" - at end of line 0, W should go to 'w' on line 1 (first word)
    let buf = Buffer::from_str("hello\nworld");
    let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWord);
    assert_eq!(result, Some(Cursor::new(1, 0))); // wraps to 'w' on line 1
}

#[test]
fn test_bigword_forward_wrap_with_leading_whitespace() {
    // "hello\n  world" - at end of line 0, W should skip the leading spaces and go to 'w' on line 1
    let buf = Buffer::from_str("hello\n  world");
    let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWord);
    assert_eq!(result, Some(Cursor::new(1, 2))); // wraps to 'w' on line 1 (skipping 2 spaces)
}

// Non-word boundary tests (bug fix for "hello---world" case)

#[test]
fn test_word_forward_with_nonword_chars() {
    // "hello---world" - at 'h', w should go to first '-' (position 5)
    let buf = Buffer::from_str("hello---world");
    let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 5))); // first '-'
}

#[test]
fn test_word_forward_at_nonword_boundary() {
    // "hello---world" - at first '-' (position 5), w should go to 'w' of "world" (position 8)
    let buf = Buffer::from_str("hello---world");
    let result = buf.next_boundary(Cursor::new(0, 5), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 8))); // first 'w' of "world"
}

#[test]
fn test_word_forward_multiple_nonword_chars() {
    // "a...b" - at 'a', w should go to first '.' (position 1)
    let buf = Buffer::from_str("a...b");
    let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 1))); // first '.'
}

#[test]
fn test_word_forward_nonword_at_start() {
    // "...hello" - at first '.' (position 0), w should go to 'h' (position 3)
    let buf = Buffer::from_str("...hello");
    let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 3))); // 'h'
}

#[test]
fn test_word_end_with_nonword_chars() {
    // "hello---world" - at 'h', e should go to 'o' (end of "hello")
    let buf = Buffer::from_str("hello---world");
    let result = buf.next_boundary(Cursor::new(0, 0), Boundary::WordEnd);
    assert_eq!(result, Some(Cursor::new(0, 4))); // 'o' (end of "hello")
}

#[test]
fn test_word_end_at_nonword_boundary() {
    // "hello---world" - at first '-' (position 5), e should go to last '-' (position 7)
    let buf = Buffer::from_str("hello---world");
    let result = buf.next_boundary(Cursor::new(0, 5), Boundary::WordEnd);
    assert_eq!(result, Some(Cursor::new(0, 7))); // last '-' (end of "---")
}

#[test]
fn test_word_backward_with_nonword_chars() {
    // "hello---world" - at 'd' (position 11), b should go to start of "world" (position 8)
    // This matches Vim behavior - b goes to start of current/previous word
    let buf = Buffer::from_str("hello---world");
    let result = buf.prev_boundary(Cursor::new(0, 11), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 8))); // start of "world"
}

#[test]
fn test_word_backward_at_nonword_boundary() {
    // "hello---world" - at first '-' (position 5), b should go to 'h' (position 0)
    let buf = Buffer::from_str("hello---world");
    let result = buf.prev_boundary(Cursor::new(0, 5), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 0))); // 'h'
}

#[test]
fn test_word_backward_at_word_boundary_after_nonword() {
    // "hello---world" - at first 'w' of "world" (position 8), b should go to first '-' (position 5)
    let buf = Buffer::from_str("hello---world");
    let result = buf.prev_boundary(Cursor::new(0, 8), Boundary::Word);
    assert_eq!(result, Some(Cursor::new(0, 5))); // first '-'
}

// BigWordEnd wrap test (bug fix for E key at end of line)

#[test]
fn test_bigword_end_at_end_of_word_wraps_to_next_line() {
    // "hello\nworld" - at end of line 0 (position 4), E should wrap to end of "world" on line 1
    let buf = Buffer::from_str("hello\nworld");
    let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWordEnd);
    assert_eq!(result, Some(Cursor::new(1, 4))); // end of "world" on line 1
}

#[test]
fn test_bigword_end_in_middle_of_word() {
    // "hello world" - at position 2 ('l'), E should go to end of "hello" (position 4)
    let buf = Buffer::from_str("hello world");
    let result = buf.next_boundary(Cursor::new(0, 2), Boundary::BigWordEnd);
    assert_eq!(result, Some(Cursor::new(0, 4))); // end of "hello"
}

#[test]
fn test_bigword_end_at_last_char_with_next_word() {
    // "hello world" - at last char of "hello" (position 4), E should go to end of "world" (position 10)
    let buf = Buffer::from_str("hello world");
    let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWordEnd);
    assert_eq!(result, Some(Cursor::new(0, 10))); // end of "world"
}

#[test]
fn test_operator_target_word_forward_range() {
    let buf = Buffer::from_str("hello world");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 6),
        }
    );
}

#[test]
fn test_operator_target_word_end_range() {
    let buf = Buffer::from_str("hello world");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 5),
        }
    );
}

#[test]
fn test_operator_target_word_backward_range() {
    let buf = Buffer::from_str("hello world");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 6),
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 6),
        }
    );
}

#[test]
fn test_operator_target_bigword_forward_range() {
    let buf = Buffer::from_str("alpha --- beta");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 6),
        }
    );
}

#[test]
fn test_operator_target_bigword_backward_range() {
    let buf = Buffer::from_str("alpha --- beta");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 10),
            OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 6),
            end: Cursor::new(0, 10),
        }
    );
}

#[test]
fn test_operator_target_line_end_range() {
    let buf = Buffer::from_str("hello world");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 6),
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 6),
            end: Cursor::new(0, 11),
        }
    );
}

#[test]
fn test_operator_target_line_start_range() {
    let buf = Buffer::from_str("hello world");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 6),
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 6),
        }
    );
}

#[test]
fn test_operator_target_line_content_start_range() {
    let buf = Buffer::from_str("    hello world");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 10),
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 4),
            end: Cursor::new(0, 10),
        }
    );
}

#[test]
fn test_operator_target_counted_word_forward_range() {
    let buf = Buffer::from_str("one two three four");
    let range = buf
        .get_operator_target_range_with_count(
            Cursor::new(0, 0),
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            2,
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 8),
        }
    );
}

#[test]
fn test_operator_target_counted_word_end_range() {
    let buf = Buffer::from_str("one two three four");
    let range = buf
        .get_operator_target_range_with_count(
            Cursor::new(0, 0),
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            2,
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 7),
        }
    );
}

#[test]
fn test_operator_target_counted_word_backward_range() {
    let buf = Buffer::from_str("one two three four");
    let range = buf
        .get_operator_target_range_with_count(
            Cursor::new(0, 8),
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            2,
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 8),
        }
    );
}

#[test]
fn test_linewise_operator_target_first_line_range() {
    let buf = Buffer::from_str("a\nb\nc\nd\ne");
    let range = buf
        .get_linewise_operator_target_range(Cursor::new(3, 0), LinewiseMotion::FirstLine)
        .unwrap();
    assert_eq!(range, LinewiseDeleteRange::new(0, 4));
}

#[test]
fn test_linewise_operator_target_last_line_range() {
    let buf = Buffer::from_str("a\nb\nc\nd\ne");
    let range = buf
        .get_linewise_operator_target_range(Cursor::new(1, 0), LinewiseMotion::LastLine)
        .unwrap();
    assert_eq!(range, LinewiseDeleteRange::new(1, 4));
}

#[test]
fn test_linewise_operator_target_counted_first_line_range() {
    let buf = Buffer::from_str("a\nb\nc\nd\ne\nf\ng\nh\ni\nj");
    let range = buf
        .get_linewise_operator_target_range_with_count(
            Cursor::new(7, 0),
            LinewiseMotion::FirstLine,
            5,
        )
        .unwrap();
    assert_eq!(range, LinewiseDeleteRange::new(4, 4));
}

#[test]
fn test_linewise_operator_target_counted_last_line_range() {
    let buf = Buffer::from_str("a\nb\nc\nd\ne\nf\ng\nh\ni\nj");
    let range = buf
        .get_linewise_operator_target_range_with_count(
            Cursor::new(2, 0),
            LinewiseMotion::LastLine,
            5,
        )
        .unwrap();
    assert_eq!(range, LinewiseDeleteRange::new(2, 3));
}

#[test]
fn test_operator_target_invalid_count_is_none() {
    let buf = Buffer::from_str("hello");
    assert!(
        buf.get_operator_target_range_with_count(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerWord),
            0,
        )
        .is_none()
    );
}

#[test]
fn test_operator_target_inner_bracket_range() {
    let buf = Buffer::from_str("foo(bar)baz");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 4),
            OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 4),
            end: Cursor::new(0, 7),
        }
    );
}

#[test]
fn test_operator_target_around_bracket_range() {
    let buf = Buffer::from_str("foo(bar)baz");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 4),
            OperatorTarget::TextObject(TextObject::AroundBracket(BracketKind::Paren)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 3),
            end: Cursor::new(0, 8),
        }
    );
}

#[test]
fn test_operator_target_bracket_range_uses_next_pair_on_current_line() {
    let buf = Buffer::from_str("x foo(bar) baz");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 6),
            end: Cursor::new(0, 9),
        }
    );
}

#[test]
fn test_operator_target_bracket_range_nested_count_expands_outward() {
    let buf = Buffer::from_str("((foo))");
    let range = buf
        .get_operator_target_range_with_count(
            Cursor::new(0, 2),
            OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
            2,
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 1),
            end: Cursor::new(0, 6),
        }
    );
}

#[test]
fn test_operator_target_bracket_range_multiline() {
    let buf = Buffer::from_str("foo(\nbar\n)baz");
    let range = buf
        .get_operator_target_range(
            Cursor::new(1, 1),
            OperatorTarget::TextObject(TextObject::AroundBracket(BracketKind::Paren)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 3),
            end: Cursor::new(2, 1),
        }
    );
}

#[test]
fn test_operator_target_bracket_range_missing_pair_is_none() {
    let buf = Buffer::from_str("foo bar");
    assert!(
        buf.get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
        )
        .is_none()
    );
}

#[test]
fn test_operator_target_inner_bracket_empty_pair_is_zero_length() {
    let buf = Buffer::from_str("()");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 1),
            end: Cursor::new(0, 1),
        }
    );
}

#[test]
fn test_operator_target_inner_quote_range() {
    let buf = Buffer::from_str("foo \"bar\" baz");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 5),
            OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 5),
            end: Cursor::new(0, 8),
        }
    );
}

#[test]
fn test_operator_target_around_quote_range() {
    let buf = Buffer::from_str("foo 'bar' baz");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 5),
            OperatorTarget::TextObject(TextObject::AroundQuote(QuoteKind::Single)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 4),
            end: Cursor::new(0, 9),
        }
    );
}

#[test]
fn test_operator_target_quote_range_uses_next_pair_on_current_line() {
    let buf = Buffer::from_str("x \"foo\" bar");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 3),
            end: Cursor::new(0, 6),
        }
    );
}

#[test]
fn test_operator_target_quote_range_ignores_escaped_delimiters() {
    let buf = Buffer::from_str("foo \"say \\\"hi\\\"\" baz");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 6),
            OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 5),
            end: Cursor::new(0, 15),
        }
    );
}

#[test]
fn test_operator_target_quote_range_multiline() {
    let buf = Buffer::from_str("\"foo\nbar\nbaz\"");
    let range = buf
        .get_operator_target_range(
            Cursor::new(1, 1),
            OperatorTarget::TextObject(TextObject::AroundQuote(QuoteKind::Double)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(2, 4),
        }
    );
}

#[test]
fn test_operator_target_quote_range_missing_pair_is_none() {
    let buf = Buffer::from_str("foo bar");
    assert!(
        buf.get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Single)),
        )
        .is_none()
    );
}

#[test]
fn test_operator_target_inner_quote_empty_pair_is_zero_length() {
    let buf = Buffer::from_str("\"\"");
    let range = buf
        .get_operator_target_range(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
        )
        .unwrap();
    assert_eq!(
        range,
        TextObjectRange {
            start: Cursor::new(0, 1),
            end: Cursor::new(0, 1),
        }
    );
}

// Delete character tests

#[test]
fn test_delete_char_before_cursor_in_middle() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 3); // at 'l'
    let new_cursor = buf.delete_char_before_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 2))); // cursor moves back
    assert_eq!(buf.as_str(), "helo");
}

#[test]
fn test_delete_char_before_cursor_at_start() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 0); // at start
    let new_cursor = buf.delete_char_before_cursor(cursor);
    assert_eq!(new_cursor, None); // nothing to delete
    assert_eq!(buf.as_str(), "hello");
}

#[test]
fn test_delete_char_before_cursor_at_doc_start() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 0);
    let new_cursor = buf.delete_char_before_cursor(cursor);
    assert_eq!(new_cursor, None);
}

#[test]
fn test_delete_char_before_cursor_joins_lines() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = Cursor::new(1, 0); // at start of line 1
    let new_cursor = buf.delete_char_before_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 5))); // at end of "hello"
    assert_eq!(buf.as_str(), "helloworld");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_delete_char_before_cursor_unicode() {
    // "héllo" - 'é' is a single grapheme (2 bytes: é = [0xc3, 0xa9])
    // Byte layout: h(0), é(1-2), l(3), l(4), o(5)
    // Cursor at byte 3 (first 'l'), should delete 'é' (bytes 1-2)
    let mut buf = Buffer::from_str("héllo");
    let cursor = Cursor::new(0, 3); // at first 'l' (byte 3)
    let new_cursor = buf.delete_char_before_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor at start of 'é'
    assert_eq!(buf.as_str(), "hllo"); // "é" removed as single unit
}

#[test]
fn test_delete_char_before_cursor_emoji() {
    // "a👍b" - emoji is 4 bytes, single grapheme
    // Byte layout: a(0), 👍(1-4), b(5)
    let mut buf = Buffer::from_str("a👍b");
    let cursor = Cursor::new(0, 5); // at 'b' (byte 5)
    let new_cursor = buf.delete_char_before_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor at 'a'
    assert_eq!(buf.as_str(), "ab"); // "👍" removed as single unit
}

#[test]
fn test_delete_char_at_cursor_in_middle() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 1); // at 'e'
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays
    assert_eq!(buf.as_str(), "hllo");
}

#[test]
fn test_delete_char_at_cursor_at_end() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 5); // at end
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, None); // nothing to delete at end
    assert_eq!(buf.as_str(), "hello");
}

#[test]
fn test_delete_char_at_cursor_at_doc_end() {
    let mut buf = Buffer::from_str("hello");
    let cursor = Cursor::new(0, 5); // at end of single line
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, None);
}

#[test]
fn test_delete_char_at_cursor_joins_lines() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = Cursor::new(0, 5); // at end of line 0
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 5))); // cursor stays at end of first line
    assert_eq!(buf.as_str(), "helloworld");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_delete_char_at_cursor_unicode() {
    // "héllo" - 'é' is a single grapheme (2 bytes)
    // Byte layout: h(0), é(1-2), l(3), l(4), o(5)
    // Cursor at byte 0 (at 'h'), should delete 'h'
    let mut buf = Buffer::from_str("héllo");
    let cursor = Cursor::new(0, 0); // at 'h' (byte 0)
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 0))); // cursor stays at start
    assert_eq!(buf.as_str(), "éllo"); // "h" removed
}

#[test]
fn test_delete_char_at_cursor_emoji() {
    // "a👍b" - emoji is 4 bytes, single grapheme
    let mut buf = Buffer::from_str("a👍b");
    let cursor = Cursor::new(0, 1); // at emoji
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays at position
    assert_eq!(buf.as_str(), "ab"); // "👍" removed as single unit
}

#[test]
fn test_delete_char_at_cursor_last_line_joins_next() {
    // When at end of last line, should try to join with next line (but none exists)
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = Cursor::new(1, 5); // at end of line 1 (last line)
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, None); // nothing to join with
    assert_eq!(buf.as_str(), "hello\nworld");
}

#[test]
fn test_delete_char_at_cursor_not_at_end_joins_next() {
    // When in middle of line, delete just removes character (no line join)
    let mut buf = Buffer::from_str("ab\ncd");
    let cursor = Cursor::new(0, 1); // at 'b' (not at end which is col 2)
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays
    assert_eq!(buf.as_str(), "a\ncd"); // 'b' removed, lines not joined
}

#[test]
fn test_insert_mode_delete_at_position_1() {
    // Simulate insert mode: cursor at position 1 in "abc"
    // This is between 'a' (pos 0) and 'b' (pos 1)
    // Delete should remove 'b' (at cursor), cursor stays at 1
    let mut buf = Buffer::from_str("abc");
    let cursor = Cursor::new(0, 1);
    let new_cursor = buf.delete_char_at_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays at position 1
    assert_eq!(buf.as_str(), "ac"); // 'b' removed
}

#[test]
fn test_insert_mode_backspace_at_position_1() {
    // Simulate insert mode: cursor at position 1 in "abc"
    // This is between 'a' (pos 0) and 'b' (pos 1)
    // Backspace should remove 'a' (before cursor), cursor moves to 0
    let mut buf = Buffer::from_str("abc");
    let cursor = Cursor::new(0, 1);
    let new_cursor = buf.delete_char_before_cursor(cursor);
    assert_eq!(new_cursor, Some(Cursor::new(0, 0))); // cursor moves back to position 0
    assert_eq!(buf.as_str(), "bc"); // 'a' removed
}

// Join lines tests

#[test]
fn test_join_lines_with_space() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = buf.join_lines(0, 2, true);
    assert_eq!(cursor, Some(Cursor::new(0, 11))); // "hello world" has 11 chars
    assert_eq!(buf.as_str(), "hello world");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_join_lines_without_space() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = buf.join_lines(0, 2, false);
    assert_eq!(cursor, Some(Cursor::new(0, 10))); // "helloworld" has 10 chars
    assert_eq!(buf.as_str(), "helloworld");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_join_lines_multiple_with_space() {
    let mut buf = Buffer::from_str("a\nb\nc\nd");
    let cursor = buf.join_lines(0, 4, true);
    assert_eq!(cursor, Some(Cursor::new(0, 7))); // "a b c d" has 7 chars
    assert_eq!(buf.as_str(), "a b c d");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_join_lines_multiple_without_space() {
    let mut buf = Buffer::from_str("a\nb\nc\nd");
    let cursor = buf.join_lines(0, 4, false);
    assert_eq!(cursor, Some(Cursor::new(0, 4))); // "abcd" has 4 chars
    assert_eq!(buf.as_str(), "abcd");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_join_lines_on_last_line_returns_none() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = buf.join_lines(1, 2, true); // Try to join from last line
    assert_eq!(cursor, None);
    assert_eq!(buf.as_str(), "hello\nworld"); // Unchanged
}

#[test]
fn test_join_lines_insufficient_lines() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = buf.join_lines(0, 5, true); // Only 2 lines available
    assert_eq!(cursor, Some(Cursor::new(0, 11))); // Still joins the 2 lines
    assert_eq!(buf.as_str(), "hello world");
}

#[test]
fn test_join_lines_with_empty_line() {
    let mut buf = Buffer::from_str("hello\n\nworld");
    // Join all 3 lines (hello, empty, world) with space
    let cursor = buf.join_lines(0, 3, true);
    assert_eq!(cursor, Some(Cursor::new(0, 12))); // "hello  world" (2 spaces) has 12 chars
    assert_eq!(buf.as_str(), "hello  world");
}

#[test]
fn test_join_lines_invalid_start_line() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = buf.join_lines(5, 2, true);
    assert_eq!(cursor, None);
}

#[test]
fn test_join_lines_count_one_returns_none() {
    let mut buf = Buffer::from_str("hello\nworld");
    let cursor = buf.join_lines(0, 1, true); // line_count < 2
    assert_eq!(cursor, None);
}

#[test]
fn test_delete_lines_single_line() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.delete_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "line2\nline3");
}

#[test]
fn test_delete_lines_multiple_lines() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
    let cursor = buf.delete_lines(0, 2);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "line3\nline4");
}

#[test]
fn test_delete_lines_from_middle() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4\nline5");
    let cursor = buf.delete_lines(1, 2);
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.as_str(), "line1\nline4\nline5");
}

#[test]
fn test_delete_lines_from_last_line() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.delete_lines(2, 1);
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "line1\nline2");
}

#[test]
fn test_delete_lines_only_line() {
    let mut buf = Buffer::from_str("only line");
    let cursor = buf.delete_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), "");
}

#[test]
fn test_delete_lines_count_exceeds_available() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.delete_lines(1, 10); // Only 2 lines from index 1
    assert_eq!(cursor, Some(Cursor::new(0, 0))); // Only line1 remains, at index 0
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), "line1");
}

#[test]
fn test_delete_lines_invalid_start_line() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.delete_lines(5, 1);
    assert_eq!(cursor, None);
}

#[test]
fn test_delete_lines_empty_buffer() {
    let mut buf = Buffer::new();
    let cursor = buf.delete_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), "");
}

#[test]
fn test_change_lines_single_line() {
    let mut buf = Buffer::from_str("hello\nworld\ntest");
    let cursor = buf.change_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.as_str(), "\nworld\ntest"); // First line replaced with blank
}

#[test]
fn test_change_lines_multiple_lines() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
    let cursor = buf.change_lines(0, 2); // Change 2 lines, leave 1 blank
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.as_str(), "\nline3\nline4"); // 2 lines replaced with 1 blank
}

#[test]
fn test_change_lines_from_middle() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
    let cursor = buf.change_lines(1, 1); // Change line 2
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.as_str(), "line1\n\nline3\nline4"); // line2 replaced with blank
}

#[test]
fn test_change_lines_from_last_line() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.change_lines(1, 1); // Change last line
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "line1\n"); // last line replaced with blank
}

#[test]
fn test_change_lines_only_line() {
    let mut buf = Buffer::from_str("only line");
    let cursor = buf.change_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), ""); // Line replaced with blank
}

#[test]
fn test_change_lines_count_exceeds_available() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.change_lines(0, 5); // Try to change 5 lines, only 3 exist
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 1); // Should leave 1 blank line
    assert_eq!(buf.as_str(), ""); // All 3 lines replaced with 1 blank
}

#[test]
fn test_change_lines_invalid_start_line() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.change_lines(5, 1); // Start beyond available lines
    assert_eq!(cursor, None);
    assert_eq!(buf.as_str(), "line1\nline2"); // No change
}

#[test]
fn test_change_lines_empty_buffer() {
    let mut buf = Buffer::new();
    let cursor = buf.change_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.as_str(), "");
}

// Tests for change_to_line_end

#[test]
fn test_change_to_line_end_middle_of_line() {
    let mut buf = Buffer::from_str("hello world");
    // Cursor after "hello" (position 5)
    let cursor = buf.change_to_line_end(Cursor::new(0, 5), 1);
    assert_eq!(cursor, Some(Cursor::new(0, 5)));
    assert_eq!(buf.as_str(), "hello");
}

#[test]
fn test_change_to_line_end_at_start_of_line() {
    let mut buf = Buffer::from_str("hello world");
    // Cursor at position 0 (before "h")
    let cursor = buf.change_to_line_end(Cursor::new(0, 0), 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.as_str(), "");
}

#[test]
fn test_change_to_line_end_at_end_of_line() {
    let mut buf = Buffer::from_str("hello");
    // Cursor at end of line (position 5)
    let cursor = buf.change_to_line_end(Cursor::new(0, 5), 1);
    assert_eq!(cursor, Some(Cursor::new(0, 5)));
    assert_eq!(buf.as_str(), "hello"); // No change
}

#[test]
fn test_change_to_line_end_multiple_lines() {
    let mut buf = Buffer::from_str("hello world\nsecond line\nthird line");
    // Cursor after "hello" (position 5 on line 0), count=2 means lines 0 and 1
    // Delete from (0,5) to end of line 1
    // Result: "hello" + "third line" (line 2 remains)
    let cursor = buf.change_to_line_end(Cursor::new(0, 5), 2);
    assert_eq!(cursor, Some(Cursor::new(0, 5)));
    assert_eq!(buf.as_str(), "hello\nthird line");
}

#[test]
fn test_change_to_line_end_count_exceeds_available() {
    let mut buf = Buffer::from_str("line1\nline2");
    // 10C on 2-line buffer should clamp to 2
    let cursor = buf.change_to_line_end(Cursor::new(0, 3), 10);
    assert_eq!(cursor, Some(Cursor::new(0, 3)));
    assert_eq!(buf.as_str(), "lin");
}

#[test]
fn test_change_to_line_end_invalid_start_line() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.change_to_line_end(Cursor::new(5, 0), 1);
    assert_eq!(cursor, None);
    assert_eq!(buf.as_str(), "line1\nline2"); // No change
}

#[test]
fn test_change_to_line_end_empty_buffer() {
    let mut buf = Buffer::new();
    let cursor = buf.change_to_line_end(Cursor::new(0, 0), 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.as_str(), "");
}

#[test]
fn test_change_to_line_end_zero_count() {
    let mut buf = Buffer::from_str("hello world");
    let cursor = buf.change_to_line_end(Cursor::new(0, 5), 0);
    assert_eq!(cursor, Some(Cursor::new(0, 5)));
    assert_eq!(buf.as_str(), "hello world"); // No change
}

#[test]
fn test_insert_lines_after_first_line() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.insert_lines_after(0, 1); // Insert after line 1
    assert_eq!(cursor, Some(Cursor::new(1, 0))); // At new line 2
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.as_str(), "line1\n\nline2\nline3");
}

#[test]
fn test_insert_lines_after_middle_line() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.insert_lines_after(1, 1); // Insert after line 2
    assert_eq!(cursor, Some(Cursor::new(2, 0))); // At new line 3
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.as_str(), "line1\nline2\n\nline3");
}

#[test]
fn test_insert_lines_after_last_line() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.insert_lines_after(1, 1); // Insert after last line
    assert_eq!(cursor, Some(Cursor::new(2, 0))); // At new line 3 (index 2)
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.as_str(), "line1\nline2\n");
}

#[test]
fn test_insert_lines_after_empty_buffer() {
    let mut buf = Buffer::new(); // Creates buffer with 1 empty line
    let cursor = buf.insert_lines_after(0, 1);
    assert_eq!(cursor, Some(Cursor::new(1, 0))); // Cursor at new line (index 1)
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "\n"); // Two empty lines
}

#[test]
fn test_insert_lines_after_multiple_lines() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.insert_lines_after(0, 3); // Insert 3 lines after line 1
    assert_eq!(cursor, Some(Cursor::new(1, 0))); // At first inserted line
    assert_eq!(buf.line_count(), 5);
    assert_eq!(buf.as_str(), "line1\n\n\n\nline2");
}

#[test]
fn test_insert_lines_after_zero_count() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.insert_lines_after(0, 0); // No lines to insert
    assert_eq!(cursor, Some(Cursor::new(0, 0))); // Cursor at line 0
    assert_eq!(buf.line_count(), 2); // No change
    assert_eq!(buf.as_str(), "line1\nline2");
}

#[test]
fn test_insert_lines_after_count_exceeds() {
    let mut buf = Buffer::from_str("line1\nline2");
    // Insert after line 5 (beyond available), should append
    let cursor = buf.insert_lines_after(5, 2);
    assert_eq!(cursor, Some(Cursor::new(2, 0))); // At first inserted line
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.as_str(), "line1\nline2\n\n");
}

#[test]
fn test_insert_lines_before_first_line() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.insert_lines_before(0, 1); // Insert before line 1
    assert_eq!(cursor, Some(Cursor::new(0, 0))); // At new line 1
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.as_str(), "\nline1\nline2\nline3");
}

#[test]
fn test_insert_lines_before_middle_line() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.insert_lines_before(1, 1); // Insert before line 2
    assert_eq!(cursor, Some(Cursor::new(1, 0))); // At new line 2
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.as_str(), "line1\n\nline2\nline3");
}

#[test]
fn test_insert_lines_before_last_line() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.insert_lines_before(1, 1); // Insert before line 2
    assert_eq!(cursor, Some(Cursor::new(1, 0))); // At new line 2
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.as_str(), "line1\n\nline2");
}

#[test]
fn test_insert_lines_before_empty_buffer() {
    let mut buf = Buffer::new();
    let cursor = buf.insert_lines_before(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "\n");
}

#[test]
fn test_insert_lines_before_multiple_lines() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.insert_lines_before(0, 3); // Insert 3 lines before line 1
    assert_eq!(cursor, Some(Cursor::new(0, 0))); // At first inserted line
    assert_eq!(buf.line_count(), 5);
    assert_eq!(buf.as_str(), "\n\n\nline1\nline2");
}

#[test]
fn test_insert_lines_before_zero_count() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.insert_lines_before(0, 0); // No lines to insert
    assert_eq!(cursor, Some(Cursor::new(0, 0))); // Cursor at line 0
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.as_str(), "line1\nline2");
}

#[test]
fn test_insert_lines_before_count_exceeds() {
    let mut buf = Buffer::from_str("line1\nline2");
    // Insert before line 5 (beyond available), should append
    let cursor = buf.insert_lines_before(5, 2);
    assert_eq!(cursor, Some(Cursor::new(2, 0))); // At first inserted line
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.as_str(), "line1\nline2\n\n");
}

// Paragraph motion tests

#[test]
fn test_is_blank_line() {
    let buf = Buffer::from_str("hello\n\n  \n\tworld\n");
    // Line 0: "hello" - not blank
    assert!(!buf.is_blank_line(0));
    // Line 1: "" - blank (empty between \n\n)
    assert!(buf.is_blank_line(1));
    // Line 2: "  " - blank (spaces only)
    assert!(buf.is_blank_line(2));
    // Line 3: "\tworld" - NOT blank (tab + "world" has non-whitespace)
    assert!(!buf.is_blank_line(3));
    // Note: trailing \n does NOT create an empty line in Rust's lines()
}

#[test]
fn test_is_blank_line_out_of_bounds() {
    let buf = Buffer::from_str("hello");
    // Out of bounds should return false
    assert!(!buf.is_blank_line(5));
    assert!(!buf.is_blank_line(100));
}

#[test]
fn test_cursor_paragraph_backward_from_paragraph() {
    // Buffer:
    // 0: "Para 1 line 1"
    // 1: "" (blank)
    // 2: "Para 2 line 1"
    // 3: "Para 2 line 2"
    // 4: "" (blank)
    // 5: "Para 3 line 1"
    let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

    // From middle of Para 2 (line 2), should find blank line before it (line 1)
    let cursor = Cursor::new(2, 5);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From line 3 (Para 2 line 2), should find blank line before Para 2 (line 1)
    let cursor = Cursor::new(3, 5);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From Para 1 (line 0), should find no previous blank line
    let cursor = Cursor::new(0, 5);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, None);
}

#[test]
fn test_cursor_paragraph_backward_from_blank_line() {
    // Buffer:
    // 0: "Para 1 line 1"
    // 1: "" (blank)
    // 2: "Para 2 line 1"
    // 3: "Para 2 line 2"
    // 4: "" (blank)
    // 5: "Para 3 line 1"
    let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

    // From blank line 1, should find blank line before Para 1 (none, returns None)
    let cursor = Cursor::new(1, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, None);

    // From blank line 4, should find blank line before Para 2 (line 1)
    let cursor = Cursor::new(4, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));
}

#[test]
fn test_cursor_paragraph_backward_multiple_paragraphs() {
    // Buffer:
    // 0: "Para 1"
    // 1: "" (blank)
    // 2: "Para 2"
    // 3: "" (blank)
    // 4: "Para 3"
    let buf = Buffer::from_str("Para 1\n\nPara 2\n\nPara 3");

    // From Para 3 (line 4), one paragraph backward
    let cursor = Cursor::new(4, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(3, 0)));

    // From Para 2 (line 2), one paragraph backward
    let cursor = Cursor::new(2, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From Para 1 (line 0), no previous paragraph
    let cursor = Cursor::new(0, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, None);
}

#[test]
fn test_cursor_paragraph_forward_from_paragraph() {
    // Buffer:
    // 0: "Para 1 line 1"
    // 1: "" (blank)
    // 2: "Para 2 line 1"
    // 3: "Para 2 line 2"
    // 4: "" (blank)
    // 5: "Para 3 line 1"
    let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

    // From Para 1 (line 0), should find blank line after Para 1 (line 1)
    let cursor = Cursor::new(0, 5);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From Para 2 line 1 (line 2), should find blank line after Para 2 (line 4)
    let cursor = Cursor::new(2, 5);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(4, 0)));
}

#[test]
fn test_cursor_paragraph_forward_from_blank_line() {
    // Buffer:
    // 0: "Para 1 line 1"
    // 1: "" (blank)
    // 2: "Para 2 line 1"
    // 3: "Para 2 line 2"
    // 4: "" (blank)
    // 5: "Para 3 line 1"
    let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

    // From blank line 1, should find blank line after Para 2 (line 4)
    let cursor = Cursor::new(1, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(4, 0)));

    // From blank line 4, should find no next paragraph (None)
    let cursor = Cursor::new(4, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, None);
}

#[test]
fn test_cursor_paragraph_forward_multiple_paragraphs() {
    // Buffer:
    // 0: "Para 1"
    // 1: "" (blank)
    // 2: "Para 2"
    // 3: "" (blank)
    // 4: "Para 3"
    let buf = Buffer::from_str("Para 1\n\nPara 2\n\nPara 3");

    // From Para 1 (line 0), one paragraph forward
    let cursor = Cursor::new(0, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From Para 2 (line 2), one paragraph forward
    let cursor = Cursor::new(2, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(3, 0)));

    // From Para 3 (line 4), no next paragraph
    let cursor = Cursor::new(4, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, None);
}

#[test]
fn test_cursor_paragraph_whitespace_only_lines() {
    // Buffer with whitespace-only lines treated as blank
    // 0: "Para 1"
    // 1: "   " (spaces - blank)
    // 2: "Para 2"
    let buf = Buffer::from_str("Para 1\n   \nPara 2");

    // From Para 1, should find blank line after it (line 1)
    let cursor = Cursor::new(0, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From Para 2, backward should find blank line before it (line 1)
    let cursor = Cursor::new(2, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));
}

#[test]
fn test_cursor_paragraph_empty_buffer() {
    let buf = Buffer::new();
    let cursor = Cursor::new(0, 0);
    assert_eq!(buf.cursor_paragraph_backward(cursor), None);
    assert_eq!(buf.cursor_paragraph_forward(cursor), None);
}

#[test]
fn test_cursor_paragraph_single_line() {
    // Buffer with only one line (not blank)
    let buf = Buffer::from_str("Single line");

    let cursor = Cursor::new(0, 5);
    assert_eq!(buf.cursor_paragraph_backward(cursor), None);
    assert_eq!(buf.cursor_paragraph_forward(cursor), None);
}
