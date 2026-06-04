use super::*;
use crate::buffer::operator_target::LinewiseDeleteRange;
use crate::config::Config;
use crate::editor::{
    BoundaryMotion, BracketKind, DelimiterFamily, LinewiseMotion, OperatorTarget, QuoteKind,
    TextObject,
};
use crate::globals::{Direction, FindKind, FindState};
use crate::path::AbsolutePath;
use crate::theme::Tag;
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

fn temp_path_with_ext(name: &str, ext: &str) -> std::path::PathBuf {
    temp_path(name).with_extension(ext)
}

fn assert_spans_include_style(spans: &[crate::buffer::syntax::SyntaxSpan], style: Tag) {
    assert!(
        spans.iter().any(|span| span.style == style),
        "expected spans to include style {style:?}"
    );
}

fn assert_spans_include_exact_style(
    spans: &[crate::buffer::syntax::SyntaxSpan],
    line: &str,
    fragment: &str,
    style: Tag,
) {
    let start_byte = line
        .find(fragment)
        .unwrap_or_else(|| panic!("expected line to contain fragment {fragment:?}"));
    let end_byte = start_byte + fragment.len();

    assert!(
        spans.iter().any(|span| {
            span.start_byte == start_byte && span.end_byte == end_byte && span.style == style
        }),
        "expected spans to include exact style {style:?} for fragment {fragment:?} at {start_byte}..{end_byte}"
    );
}

fn assert_exact_spans(
    spans: &[crate::buffer::syntax::SyntaxSpan],
    expected: &[(usize, usize, Tag)],
) {
    assert_eq!(
        spans.len(),
        expected.len(),
        "expected exactly {} spans, got {}",
        expected.len(),
        spans.len()
    );

    for (span, (start, end, style)) in spans.iter().zip(expected.iter()) {
        assert_eq!(
            (span.start_byte, span.end_byte, &span.style),
            (*start, *end, style)
        );
    }
}

fn assert_spans_include_comment_style(spans: &[crate::buffer::syntax::SyntaxSpan]) {
    assert!(
        spans.iter().any(|span| {
            span.style == tag("comment")
                || span.style == tag("comment.line")
                || span.style == tag("comment.block")
                || span.style == tag("comment.documentation")
        }),
        "expected spans to include a comment style"
    );
}

fn tag(value: &str) -> Tag {
    Tag::parse(value).expect("valid tag")
}

fn fixture_buffer(name: &str, ext: &str, contents: &str) -> Buffer {
    let path = AbsolutePath::from_path(temp_path_with_ext(name, ext).as_path()).unwrap();
    Buffer::from_str_with_path(contents, path)
}

fn named_fixture_buffer(name: &str, contents: &str) -> Buffer {
    let path = AbsolutePath::from_path(std::env::temp_dir().join(name).as_path()).unwrap();
    Buffer::from_str_with_path(contents, path)
}

fn line_containing(buf: &Buffer, needle: &str) -> usize {
    (0..buf.line_count())
        .find(|line| {
            buf.line_at(*line)
                .is_some_and(|line_text| line_text.to_string().contains(needle))
        })
        .unwrap_or_else(|| panic!("fixture should contain {needle:?}"))
}

fn syntax_buffer(name: &str, ext: &str, contents: &str) -> Buffer {
    let path = AbsolutePath::from_path(temp_path_with_ext(name, ext).as_path()).unwrap();
    Buffer::from_str_with_path(contents, path)
}

fn line_spans(buf: &mut Buffer, line: usize) -> Vec<crate::buffer::syntax::SyntaxSpan> {
    buf.syntax_spans_for_line(line)
        .expect("line should exist")
        .to_vec()
}

fn assert_buffer_eq(buf: &Buffer, expected: &str, lines: usize) {
    assert_eq!(buf.as_str(), expected);
    assert_eq!(buf.line_count(), lines);
}

fn tab_config_4() -> crate::globals::TestConfigGuard {
    crate::globals::set_test_config(Config {
        tab_width: 4,
        ..Default::default()
    })
}

fn three_para_buffer() -> Buffer {
    Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1")
}

macro_rules! assert_next_boundary {
    ($name:ident, $text:expr, $cursor:expr, $boundary:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let result = buf.next_boundary($cursor, $boundary);
            assert_eq!(result, $expected);
        }
    };
    ($name:ident, $text:expr, $cursor:expr, $boundary:expr, $expected:expr, $msg:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let result = buf.next_boundary($cursor, $boundary);
            assert_eq!(result, $expected, $msg);
        }
    };
}

macro_rules! assert_prev_boundary {
    ($name:ident, $text:expr, $cursor:expr, $boundary:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let result = buf.prev_boundary($cursor, $boundary);
            assert_eq!(result, $expected);
        }
    };
    ($name:ident, $text:expr, $cursor:expr, $boundary:expr, $expected:expr, $msg:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let result = buf.prev_boundary($cursor, $boundary);
            assert_eq!(result, $expected, $msg);
        }
    };
}

macro_rules! assert_operator_range {
    ($name:ident, $text:expr, $cursor:expr, $target:expr, $start:expr, $end:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let range = buf.get_operator_target_range($cursor, $target).unwrap();
            assert_eq!(
                range,
                TextObjectRange {
                    start: $start,
                    end: $end,
                }
            );
        }
    };
}

macro_rules! assert_operator_range_with_count {
    ($name:ident, $text:expr, $cursor:expr, $target:expr, $count:expr, $start:expr, $end:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let range = buf
                .get_operator_target_range_with_count($cursor, $target, $count)
                .unwrap();
            assert_eq!(
                range,
                TextObjectRange {
                    start: $start,
                    end: $end,
                }
            );
        }
    };
}

macro_rules! assert_linewise_range {
    ($name:ident, $text:expr, $cursor:expr, $motion:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let range = buf
                .get_linewise_operator_target_range($cursor, $motion)
                .unwrap();
            assert_eq!(range, $expected);
        }
    };
}

macro_rules! assert_linewise_range_with_count {
    ($name:ident, $text:expr, $cursor:expr, $motion:expr, $count:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let buf = Buffer::from_str($text);
            let range = buf
                .get_linewise_operator_target_range_with_count($cursor, $motion, $count)
                .unwrap();
            assert_eq!(range, $expected);
        }
    };
}

mod markers;
mod syntax;

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
fn test_syntax_name_from_shebang() {
    let buf = Buffer::from_str("#!/usr/bin/env python3 -O\nprint('hello')");
    assert_eq!(buf.syntax_name(), "python");
}

#[test]
fn test_syntax_name_from_filename() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.php")).unwrap();
    let buf = Buffer::from_str_with_path("<?php echo 'hello';", path);

    assert_eq!(buf.syntax_name(), "php");
}

#[test]
fn test_comment_prefix_from_syntax() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("comment-prefix", "rs").as_path()).unwrap();
    let buf = Buffer::from_str_with_path("fn main() {}", path);

    assert_eq!(buf.comment_prefix().as_deref(), Some("//"));
}

#[test]
fn test_syntax_name_shebang_takes_precedence_over_filename() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.rs")).unwrap();
    let buf = Buffer::from_str_with_path("#!/usr/bin/env python3\nprint('hello')", path);

    assert_eq!(buf.syntax_name(), "python");
}

#[test]
fn test_syntax_name_updates_after_first_line_edit() {
    let mut buf = Buffer::from_str("#!/usr/bin/env python3\nprint('hello')");

    assert_eq!(buf.syntax_name(), "python");

    let shebang_len = buf.line_len(0);
    buf.remove(Cursor::new(0, 0), Cursor::new(0, shebang_len));
    buf.insert_text(Cursor::new(0, 0), "plain text");

    assert_eq!(buf.syntax_name(), "plaintext");
    buf.mark_saved();
    assert_eq!(buf.syntax_name(), "plaintext");
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
fn test_toggle_line_comment_comments_and_uncomments() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("toggle-comment", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("fn main() {}", path);
    let prefix = buf
        .comment_prefix()
        .expect("rust should define a comment prefix");

    let cursor = buf
        .toggle_line_comment(Cursor::new(0, 0), &prefix)
        .expect("toggle should comment the line");
    assert_eq!(buf.test_line_str(0).as_deref(), Some("// fn main() {}"));
    assert_eq!(cursor, Cursor::new(0, 3));

    let cursor = buf
        .toggle_line_comment(cursor, &prefix)
        .expect("toggle should uncomment the line");
    assert_eq!(buf.test_line_str(0).as_deref(), Some("fn main() {}"));
    assert_eq!(cursor, Cursor::new(0, 0));
}

#[test]
fn test_toggle_line_comment_preserves_indentation() {
    let path = AbsolutePath::from_path(temp_path_with_ext("toggle-comment-indent", "py").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("    print('hello')", path);
    let prefix = buf
        .comment_prefix()
        .expect("python should define a comment prefix");

    let cursor = buf
        .toggle_line_comment(Cursor::new(0, 4), &prefix)
        .expect("toggle should comment the indented line");
    assert_eq!(
        buf.test_line_str(0).as_deref(),
        Some("    # print('hello')")
    );
    assert_eq!(cursor, Cursor::new(0, 6));
}

#[test]
fn test_toggle_line_comment_aligns_to_minimum_column_across_range() {
    let path = AbsolutePath::from_path(temp_path_with_ext("toggle-comment-align", "rs").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("  fn a() {}\n    fn b() {}", path);
    let prefix = buf
        .comment_prefix()
        .expect("rust should define a comment prefix");

    let cursor = buf
        .toggle_line_comments(Cursor::new(0, 0), 2, &prefix)
        .expect("toggle should comment the range");
    assert_eq!(buf.test_line_str(0).as_deref(), Some("  // fn a() {}"));
    assert_eq!(buf.test_line_str(1).as_deref(), Some("  //   fn b() {}"));
    assert_eq!(cursor, Cursor::new(0, 0));
}

#[test]
fn test_toggle_line_comment_skips_blank_lines() {
    let path = AbsolutePath::from_path(temp_path_with_ext("toggle-comment-blank", "py").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("\n    print('hello')", path);
    let prefix = buf
        .comment_prefix()
        .expect("python should define a comment prefix");

    let cursor = buf
        .toggle_line_comments(Cursor::new(0, 0), 2, &prefix)
        .expect("toggle should succeed");
    assert_eq!(buf.test_line_str(0).as_deref(), Some(""));
    assert_eq!(
        buf.test_line_str(1).as_deref(),
        Some("    # print('hello')")
    );
    assert_eq!(cursor, Cursor::new(0, 0));
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
fn test_insert_text_increments_syntax_generation_once() {
    let mut buf = Buffer::from_str("hello");
    let before = buf.syntax_generation();

    buf.insert_text(Cursor::new(0, 5), " world");

    assert_eq!(buf.syntax_generation(), before + 1);
}

#[test]
fn test_add_surround_wraps_character_range() {
    let mut buf = Buffer::from_str("hello world");
    let cursor = buf
        .add_surround(
            TextObjectRange {
                start: Cursor::new(0, 0),
                end: Cursor::new(0, 5),
            },
            DelimiterFamily::DoubleQuote,
        )
        .expect("surround add should succeed");

    assert_eq!(buf.as_str(), "\"hello\" world");
    assert_eq!(cursor, Cursor::new(0, 0));
}

#[test]
fn test_add_surround_rejects_empty_range() {
    let mut buf = Buffer::from_str("hello");
    assert_eq!(
        buf.add_surround(
            TextObjectRange {
                start: Cursor::new(0, 2),
                end: Cursor::new(0, 2),
            },
            DelimiterFamily::Paren,
        ),
        None
    );
    assert_eq!(buf.as_str(), "hello");
}

#[test]
fn test_add_linewise_surround_wraps_selected_lines() {
    let mut buf = Buffer::from_str("alpha\nbeta\ngamma");
    let cursor = buf
        .add_linewise_surround(1, 1, DelimiterFamily::Curly)
        .expect("linewise surround add should succeed");

    assert_eq!(buf.as_str(), "alpha\n{\nbeta\n}\ngamma");
    assert_eq!(cursor, Cursor::new(1, 0));
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
fn test_insert_text_with_newline_increments_syntax_generation_once() {
    let mut buf = Buffer::from_str("hello");
    let before = buf.syntax_generation();

    buf.insert_text(Cursor::new(0, 2), "X\nY");

    assert_eq!(buf.syntax_generation(), before + 1);
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
    assert_eq!(buf.test_line_str(0).as_deref(), Some("line1"));
    assert_eq!(buf.test_line_str(1).as_deref(), Some("line2"));
    assert_eq!(buf.test_line_str(2).as_deref(), Some("line3"));
}

#[test]
fn test_line_at_out_of_bounds() {
    let buf = Buffer::from_str("hello");
    assert!(buf.line_at(1).is_none());
}

#[test]
fn test_line_grapheme_len() {
    let buf = Buffer::from_str("a😀c\n");
    assert_eq!(buf.line_at(0).map(|s| str_width(&s.to_text())), Some(4));
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
fn test_delete_lines_shifts_ghost_texts_with_surviving_lines() {
    let mut buf = Buffer::from_str("line0\nline1\nline2");
    let ghost_1 = buf.insert_ghost_text(Cursor::new(1, 2), Gravity::Right, "g1");
    let ghost_2 = buf.insert_ghost_text(Cursor::new(2, 3), Gravity::Right, "g2");

    let cursor = buf
        .delete_lines(0, 1)
        .expect("delete_lines should return a cursor");

    assert_eq!(cursor, Cursor::new(0, 0));
    assert_eq!(buf.line_count(), 2);

    let line_0 = buf.ghost_texts_for_line(0).expect("line 0 should exist");
    assert_eq!(line_0.len(), 1);
    assert_eq!(line_0[0].id, ghost_1);

    let line_1 = buf.ghost_texts_for_line(1).expect("line 1 should exist");
    assert_eq!(line_1.len(), 1);
    assert_eq!(line_1[0].id, ghost_2);
}

#[test]
fn test_delete_lines_drops_ghost_texts_from_deleted_lines() {
    let mut buf = Buffer::from_str("line0\nline1\nline2");
    let deleted_ghost = buf.insert_ghost_text(Cursor::new(0, 2), Gravity::Right, "gone");
    let kept_ghost = buf.insert_ghost_text(Cursor::new(2, 3), Gravity::Right, "stay");

    buf.delete_lines(0, 1)
        .expect("delete_lines should return a cursor");

    assert!(buf.ghost_text(deleted_ghost).is_none());

    let line_1 = buf.ghost_texts_for_line(1).expect("line 1 should exist");
    assert_eq!(line_1.len(), 1);
    assert_eq!(line_1[0].id, kept_ghost);
}

#[test]
fn test_delete_lines_undo_restores_ghost_text_positions() {
    let mut buf = Buffer::from_str("line0\nline1\nline2");
    let ghost_0 = buf.insert_ghost_text(Cursor::new(0, 2), Gravity::Right, "g0");
    let ghost_1 = buf.insert_ghost_text(Cursor::new(1, 2), Gravity::Right, "g1");
    let ghost_2 = buf.insert_ghost_text(Cursor::new(2, 2), Gravity::Right, "g2");

    buf.push_snapshot(Cursor::new(0, 0));
    buf.delete_lines(0, 1)
        .expect("delete_lines should return a cursor");
    buf.push_snapshot(Cursor::new(0, 0));

    buf.undo().expect("undo should restore the prior snapshot");

    assert_eq!(buf.line_count(), 3);
    assert_eq!(
        buf.ghost_text(ghost_0).unwrap().kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(0, 2),
            gravity: Gravity::Right,
        })
    );
    assert_eq!(
        buf.ghost_text(ghost_1).unwrap().kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(1, 2),
            gravity: Gravity::Right,
        })
    );
    assert_eq!(
        buf.ghost_text(ghost_2).unwrap().kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(2, 2),
            gravity: Gravity::Right,
        })
    );
}

#[test]
fn test_save_buffer_clears_modified_state_and_refreshes_syntax() {
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
    assert_eq!(pool.get(id).unwrap().syntax_name(), "plaintext");

    pool.save_buffer(id).unwrap();

    assert!(!pool.get(id).unwrap().is_modified());
    assert_eq!(pool.get(id).unwrap().syntax_name(), "plaintext");

    fs::remove_file(&path).ok();
}

#[test]
fn test_syntax_spans_for_supported_filetype() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-supported", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path(
        "fn main() { let value: Option<String> = Some(\"hi\"); } // note",
        path,
    );

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert!(spans.iter().any(|span| span.style == tag("keyword")));
    assert!(spans.iter().any(|span| span.style == tag("constant")));
    assert!(spans.iter().any(|span| span.style == tag("type")));
    assert!(spans.iter().any(|span| span.style == tag("function")));
    assert!(spans.iter().any(|span| span.style == tag("string")));
    assert_spans_include_comment_style(&spans);
}

#[test]
fn test_syntax_spans_update_after_edit() {
    let mut buf = syntax_buffer("syntax-edit", "rs", "let value = 1;");

    assert!(
        line_spans(&mut buf, 0)
            .iter()
            .any(|span| span.style == tag("keyword"))
    );
    assert!(
        line_spans(&mut buf, 0)
            .iter()
            .any(|span| span.style == tag("variable"))
    );

    buf.insert_text(Cursor::new(0, 0), "// ");

    assert!(!buf.syntax_cache_complete());
    assert!(!buf.syntax_background_pending());
    assert!(buf.indent_scope_cache_stale());

    let spans = line_spans(&mut buf, 0);
    assert_spans_include_comment_style(&spans);
    assert!(buf.syntax_cache_complete());
}

#[test]
fn test_syntax_spans_update_after_full_replace() {
    let mut buf = syntax_buffer("syntax-replace", "rs", "let value = 1;");

    assert!(
        line_spans(&mut buf, 0)
            .iter()
            .any(|span| span.style == tag("keyword"))
    );

    buf.replace_text("// let value = 1;\nlet next = 2;");

    assert_eq!(buf.cached_syntax_line_count(), 0);
    assert!(!buf.syntax_cache_complete());
    let first_line = line_spans(&mut buf, 0);
    let second_line = line_spans(&mut buf, 1);

    assert_spans_include_comment_style(&first_line);
    assert!(second_line.iter().any(|span| span.style == tag("keyword")));
}

#[test]
fn test_syntax_spans_update_after_full_replace_same_line_length_change() {
    let mut buf = syntax_buffer(
        "syntax-replace-same-line",
        "rs",
        "let config = Config::load(cli.theme.as_deref(), cli.no_syntax.then_some(false));",
    );

    let original = line_spans(&mut buf, 0);
    assert_spans_include_exact_style(
        &original,
        "let config = Config::load(cli.theme.as_deref(), cli.no_syntax.then_some(false));",
        "Config",
        tag("type"),
    );

    buf.replace_text(
        "let configg = Config::load(cli.theme.as_deref(), cli.no_syntax.then_some(false));",
    );

    assert_eq!(buf.cached_syntax_line_count(), 0);
    let updated = line_spans(&mut buf, 0);
    assert_spans_include_exact_style(
        &updated,
        "let configg = Config::load(cli.theme.as_deref(), cli.no_syntax.then_some(false));",
        "Config",
        tag("type"),
    );
}

#[test]
fn test_syntax_spans_preserve_multiline_state() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-multiline", "toml").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("value = \"\"\"hello\nworld\"\"\"", path);

    let first_line = buf
        .syntax_spans_for_line(0)
        .expect("first line should exist");
    let second_line = buf
        .syntax_spans_for_line(1)
        .expect("second line should exist");

    assert!(first_line.iter().any(|span| span.style == tag("string")));
    assert!(second_line.iter().any(|span| span.style == tag("string")));
}

#[test]
fn test_multiline_string_reuses_downstream_syntax_after_line_insertion() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-multiline-insert", "toml").as_path())
            .unwrap();
    let mut buf =
        Buffer::from_str_with_path("value = \"\"\"hello\nplanet\nworld\"\"\"\nafter", path);

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    buf.insert_lines_before(1, 1);

    let closing = buf
        .syntax_spans_for_line(3)
        .expect("closing line should exist");
    let after = buf
        .syntax_spans_for_line(4)
        .expect("tail line should exist");

    assert!(closing.iter().any(|span| span.style == tag("string")));
    assert!(!after.iter().any(|span| span.style == tag("string")));
}

#[test]
fn test_markdown_code_fence_injects_nested_syntax() {
    let mut buf = syntax_buffer(
        "syntax-fence",
        "md",
        "```rust\nfn main() { let value = Some(\"hi\"); }\n```",
    );

    let first_line = line_spans(&mut buf, 0);
    let second_line = line_spans(&mut buf, 1);
    let third_line = line_spans(&mut buf, 2);

    assert!(
        first_line
            .iter()
            .any(|span| span.style == tag("markup.code"))
    );
    assert!(second_line.iter().any(|span| span.style == tag("keyword")));
    assert!(second_line.iter().any(|span| span.style == tag("constant")));
    assert!(second_line.iter().any(|span| span.style == tag("variable")));
    assert!(second_line.iter().any(|span| span.style == tag("string")));
    assert!(
        third_line
            .iter()
            .any(|span| span.style == tag("markup.code"))
    );
}

#[test]
fn test_markdown_code_fence_updates_after_opening_language_edit() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-fence-edit", "md").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path(
        "```rust\nfn main() { let value = Some(\"hi\"); }\n```",
        path,
    );

    let initial_body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    assert_spans_include_style(&initial_body, tag("keyword"));
    assert_spans_include_style(&initial_body, tag("string"));

    buf.remove(Cursor::new(0, 3), Cursor::new(0, 7));
    buf.insert_text(Cursor::new(0, 3), "wat");

    let updated_body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    let closing = buf
        .syntax_spans_for_line(2)
        .expect("closing line should exist");

    assert!(updated_body.is_empty());
    assert_eq!(closing.len(), 1);
    assert_eq!(closing[0].style, tag("markup.code"));
}

#[test]
fn test_javascript_types_use_type_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-type", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("class Thing extends Error {}", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("keyword"));
    assert_spans_include_style(&spans, tag("type"));
}

#[test]
fn test_markdown_code_fence_resolves_canonical_capture() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-fence-js", "md").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path(
        "```javascript\nconst value = null; const count = 1;\n```",
        path,
    );

    let body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    assert!(body.iter().any(|span| span.style == tag("keyword")));
    assert!(body.iter().any(|span| span.style == tag("constant")));
    assert!(body.iter().any(|span| span.style == tag("variable")));
    assert!(body.iter().any(|span| span.style == tag("number")));
}

#[test]
fn test_markdown_code_fence_resolves_alias_capture() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-fence-js-alias", "md").as_path())
        .unwrap();
    let mut buf =
        Buffer::from_str_with_path("```js\nconst value = null; const count = 1;\n```", path);

    let body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    assert!(body.iter().any(|span| span.style == tag("keyword")));
    assert!(body.iter().any(|span| span.style == tag("constant")));
    assert!(body.iter().any(|span| span.style == tag("variable")));
    assert!(body.iter().any(|span| span.style == tag("number")));
}

#[test]
fn test_markdown_fixture_js_closing_fence_uses_code_block_tag() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-fixture", "md").as_path()).unwrap();
    let fixture = include_str!("tests/syntax/fixtures/markdown.md");
    let mut buf = Buffer::from_str_with_path(fixture, path);

    let closing = buf
        .syntax_spans_for_line(22)
        .expect("fixture closing fence should exist");
    assert_eq!(closing.len(), 1);
    assert_eq!(closing[0].style, tag("markup.code"));
}

#[test]
fn test_json_fixture_uses_grammar_rules() {
    let fixture = include_str!("tests/syntax/fixtures/json.json");
    let mut buf = fixture_buffer("syntax-json-fixture", "json", fixture);

    let line_one = buf.syntax_spans_for_line(1).expect("line should exist");
    let line_three = buf.syntax_spans_for_line(3).expect("line should exist");
    let line_four = buf.syntax_spans_for_line(4).expect("line should exist");
    let line_five = buf.syntax_spans_for_line(5).expect("line should exist");

    assert_spans_include_style(&line_one, tag("string"));
    assert_spans_include_style(&line_one, tag("punctuation"));
    assert_spans_include_style(&line_three, tag("constant"));
    assert_spans_include_style(&line_three, tag("punctuation"));
    assert_spans_include_style(&line_four, tag("number"));
    assert_spans_include_style(&line_four, tag("punctuation"));
    assert_spans_include_style(&line_five, tag("punctuation"));
}

#[test]
fn test_toml_fixture_uses_grammar_rules() {
    let fixture = include_str!("tests/syntax/fixtures/toml.toml");
    let mut buf = fixture_buffer("syntax-toml-fixture", "toml", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let string_line = buf
        .syntax_spans_for_line(1)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(2)
        .expect("number line should exist");
    let bool_line = buf
        .syntax_spans_for_line(3)
        .expect("bool line should exist");
    let table_line = buf
        .syntax_spans_for_line(5)
        .expect("table line should exist");
    let array_line = buf
        .syntax_spans_for_line(11)
        .expect("array line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&string_line, tag("operator"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&number_line, tag("operator"));
    assert_spans_include_style(&bool_line, tag("constant"));
    assert_spans_include_style(&bool_line, tag("operator"));
    assert_spans_include_style(&table_line, tag("keyword"));
    assert_spans_include_style(&array_line, tag("number"));
    assert_spans_include_style(&array_line, tag("punctuation"));
}

#[test]
fn test_javascript_fixture_uses_grammar_rules() {
    let fixture = include_str!("tests/syntax/fixtures/javascript.js");
    let mut buf = fixture_buffer("syntax-js-fixture", "js", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let keyword_line = buf
        .syntax_spans_for_line(3)
        .expect("keyword line should exist");
    let object_line = buf
        .syntax_spans_for_line(5)
        .expect("object line should exist");
    let operator_line = buf
        .syntax_spans_for_line(12)
        .expect("operator line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&keyword_line, tag("keyword"));
    assert_spans_include_style(&keyword_line, tag("punctuation"));
    assert_spans_include_style(&object_line, tag("punctuation"));
    assert_spans_include_style(&object_line, tag("constant"));
    assert_spans_include_style(&operator_line, tag("operator"));
    assert_spans_include_style(&operator_line, tag("constant"));
}

#[test]
fn test_rust_fixture_uses_grammar_rules() {
    let fixture = include_str!("tests/syntax/fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-fixture", "rs", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let function_line = buf
        .syntax_spans_for_line(3)
        .expect("function line should exist");
    let function_line_text = buf
        .line_at(3)
        .expect("function line should exist")
        .to_string();
    let type_line = buf
        .syntax_spans_for_line(4)
        .expect("type line should exist");
    let block_line = buf
        .syntax_spans_for_line(14)
        .expect("block line should exist");
    let block_line_text = buf
        .line_at(14)
        .expect("block line should exist")
        .to_string();
    let operator_line = buf
        .syntax_spans_for_line(12)
        .expect("operator line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_exact_style(&function_line, &function_line_text, "{", tag("punctuation"));
    assert_spans_include_style(&type_line, tag("type"));
    assert_spans_include_style(&type_line, tag("punctuation"));
    assert_spans_include_style(&type_line, tag("operator"));
    assert_spans_include_style(&operator_line, tag("operator"));
    assert_spans_include_style(&operator_line, tag("keyword"));
    assert_spans_include_style(&operator_line, tag("punctuation"));
    assert_spans_include_exact_style(&block_line, &block_line_text, "}", tag("punctuation"));
    assert_spans_include_exact_style(&block_line, &block_line_text, "{", tag("punctuation"));
}

#[test]
fn test_python_fixture_uses_grammar_rules() {
    let fixture = include_str!("tests/syntax/fixtures/python.py");
    let mut buf = fixture_buffer("syntax-py-fixture", "py", fixture);

    let docstring = buf
        .syntax_spans_for_line(1)
        .expect("docstring line should exist");
    let comment = buf
        .syntax_spans_for_line(6)
        .expect("comment line should exist");
    let definition = buf
        .syntax_spans_for_line(8)
        .expect("definition line should exist");
    let mapping = buf
        .syntax_spans_for_line(21)
        .expect("mapping line should exist");

    assert_spans_include_style(&docstring, tag("string"));
    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&definition, tag("keyword"));
    assert_spans_include_style(&definition, tag("type"));
    assert_spans_include_style(&definition, tag("punctuation"));
    assert_spans_include_style(&definition, tag("operator"));
    assert_spans_include_style(&mapping, tag("punctuation"));
    assert_spans_include_style(&mapping, tag("constant"));
}

#[test]
fn test_shell_fixture_uses_grammar_rules() {
    let fixture = include_str!("tests/syntax/fixtures/shell.sh");
    let mut buf = fixture_buffer("syntax-shell-fixture", "sh", fixture);

    let comment = buf
        .syntax_spans_for_line(1)
        .expect("comment line should exist");
    let function_line = buf
        .syntax_spans_for_line(3)
        .expect("function line should exist");
    let assignment_line = buf
        .syntax_spans_for_line(4)
        .expect("assignment line should exist");
    let keyword_line = buf
        .syntax_spans_for_line(9)
        .expect("keyword line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&function_line, tag("punctuation"));
    assert_spans_include_style(&assignment_line, tag("type"));
    assert_spans_include_style(&assignment_line, tag("operator"));
    assert_spans_include_style(&assignment_line, tag("string"));
    assert_spans_include_style(&keyword_line, tag("keyword"));
    assert_spans_include_style(&keyword_line, tag("constant"));
    assert_spans_include_style(&keyword_line, tag("punctuation"));
}

#[test]
fn test_rust_fixture_highlights_extended_literals() {
    let fixture = include_str!("tests/syntax/fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-extended", "rs", fixture);

    let doc_comment_idx = line_containing(&buf, "/// Doc comment");
    let attribute_idx = line_containing(&buf, "#[inline]");
    let raw_string_idx = line_containing(&buf, "raw = r#");
    let raw_multiline_idx = line_containing(&buf, "let raw_multiline");
    let byte_string_idx = line_containing(&buf, "bytes = b");
    let raw_bytes_idx = line_containing(&buf, "raw_bytes");
    let numeric_idx = line_containing(&buf, "hex = 0xff_u8");

    let doc_comment = buf
        .syntax_spans_for_line(doc_comment_idx)
        .expect("doc comment line should exist");
    let attribute = buf
        .syntax_spans_for_line(attribute_idx)
        .expect("attribute line should exist");
    let raw_string = buf
        .syntax_spans_for_line(raw_string_idx)
        .expect("raw string line should exist");
    let raw_multiline = buf
        .syntax_spans_for_line(raw_multiline_idx)
        .expect("raw multiline line should exist");
    let byte_string = buf
        .syntax_spans_for_line(byte_string_idx)
        .expect("byte string line should exist");
    let raw_bytes = buf
        .syntax_spans_for_line(raw_bytes_idx)
        .expect("raw byte string line should exist");
    let numeric = buf
        .syntax_spans_for_line(numeric_idx)
        .expect("numeric line should exist");

    assert_spans_include_style(&doc_comment, tag("comment.documentation"));
    assert_spans_include_style(&attribute, tag("punctuation"));
    assert_spans_include_style(&raw_string, tag("string"));
    assert_spans_include_style(&raw_multiline, tag("string"));
    assert_spans_include_style(&byte_string, tag("string"));
    assert_spans_include_style(&raw_bytes, tag("string"));
    assert_spans_include_style(&numeric, tag("number"));
    assert!(raw_multiline.iter().any(|span| span.style == tag("string")));
}

#[test]
fn test_python_fixture_highlights_extended_prefixes() {
    let fixture = include_str!("tests/syntax/fixtures/python.py");
    let mut buf = fixture_buffer("syntax-python-extended", "py", fixture);

    let decorator = buf
        .syntax_spans_for_line(28)
        .expect("decorator line should exist");
    let raw_string = buf
        .syntax_spans_for_line(30)
        .expect("raw string line should exist");
    let bytes_string = buf
        .syntax_spans_for_line(31)
        .expect("bytes string line should exist");
    let raw_bytes = buf
        .syntax_spans_for_line(33)
        .expect("raw bytes line should exist");
    let combined = buf
        .syntax_spans_for_line(34)
        .expect("combined f-string line should exist");
    let raw_combined = buf
        .syntax_spans_for_line(35)
        .expect("raw combined f-string line should exist");
    let numeric = buf
        .syntax_spans_for_line(37)
        .expect("numeric line should exist");

    assert_spans_include_style(&decorator, tag("keyword"));
    assert_spans_include_style(&raw_string, tag("string"));
    assert_spans_include_style(&bytes_string, tag("string"));
    assert_spans_include_style(&raw_bytes, tag("string"));
    assert_spans_include_style(&combined, tag("string"));
    assert_spans_include_style(&raw_combined, tag("string"));
    assert_spans_include_style(&numeric, tag("number"));
}

#[test]
fn test_python_raw_fstring_highlights_interpolation_body() {
    let mut buf = fixture_buffer(
        "syntax-python-raw-fstring",
        "py",
        r#"value = rf"hello {name}\n""#,
    );
    let spans = buf
        .syntax_spans_for_line(0)
        .expect("raw f-string line should exist");

    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("variable"));
}

#[test]
fn test_javascript_fixture_highlights_regex_private_fields_and_bigints() {
    let fixture = include_str!("tests/syntax/fixtures/javascript.js");
    let mut buf = fixture_buffer("syntax-js-extended", "js", fixture);

    let regex_line = buf
        .syntax_spans_for_line(19)
        .expect("regex line should exist");
    let bigint_line = buf
        .syntax_spans_for_line(20)
        .expect("bigint line should exist");
    let private_field = buf
        .syntax_spans_for_line(26)
        .expect("private field line should exist");
    let private_access = buf
        .syntax_spans_for_line(28)
        .expect("private access line should exist");

    assert_spans_include_style(&regex_line, tag("string"));
    assert_spans_include_style(&bigint_line, tag("number"));
    assert_spans_include_style(&private_field, tag("variable"));
    assert_spans_include_style(&private_access, tag("variable"));
}

#[test]
fn test_json_fixture_rejects_identifier_like_text() {
    let fixture = include_str!("tests/syntax/fixtures/json.json");
    let mut buf = fixture_buffer("syntax-json-extended", "json", fixture);

    let negative = buf
        .syntax_spans_for_line(12)
        .expect("negative number line should exist");
    let identifier = buf
        .syntax_spans_for_line(13)
        .expect("identifier-like line should exist");

    assert_spans_include_style(&negative, tag("number"));
    assert!(identifier.is_empty());
}

#[test]
fn test_toml_fixture_highlights_tables_and_extended_numbers() {
    let fixture = include_str!("tests/syntax/fixtures/toml.toml");
    let mut buf = fixture_buffer("syntax-toml-extended", "toml", fixture);

    let dotted_key = buf
        .syntax_spans_for_line(17)
        .expect("dotted key line should exist");
    let table = buf
        .syntax_spans_for_line(19)
        .expect("table line should exist");
    let base_numbers = buf
        .syntax_spans_for_line(20)
        .expect("base number line should exist");
    let inline_table = buf
        .syntax_spans_for_line(21)
        .expect("inline table line should exist");
    let array_of_tables = buf
        .syntax_spans_for_line(23)
        .expect("array of tables line should exist");

    assert_spans_include_style(&dotted_key, tag("variable"));
    assert_spans_include_style(&dotted_key, tag("operator"));
    assert_spans_include_style(&table, tag("keyword"));
    assert_spans_include_style(&base_numbers, tag("number"));
    assert_spans_include_style(&inline_table, tag("number"));
    assert_spans_include_style(&inline_table, tag("constant"));
    assert_spans_include_style(&array_of_tables, tag("keyword"));
}

#[test]
fn test_markdown_fixture_highlights_extended_structures() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-markdown-extended", "md").as_path())
            .unwrap();
    let fixture = include_str!("tests/syntax/fixtures/markdown.md");
    let mut buf = Buffer::from_str_with_path(fixture, path);

    let setext = buf
        .syntax_spans_for_line(31)
        .expect("setext underline line should exist");
    let reference_link = buf
        .syntax_spans_for_line(33)
        .expect("reference link line should exist");
    let reference_definition = buf
        .syntax_spans_for_line(34)
        .expect("reference definition line should exist");
    let autolink = buf
        .syntax_spans_for_line(37)
        .expect("autolink line should exist");
    let underscore_line = buf
        .syntax_spans_for_line(39)
        .expect("underscore emphasis line should exist");
    let indented_code = buf
        .syntax_spans_for_line(41)
        .expect("indented code line should exist");
    let tilde_fence = buf
        .syntax_spans_for_line(43)
        .expect("tilde fence line should exist");
    let tilde_body = buf
        .syntax_spans_for_line(44)
        .expect("tilde fence body line should exist");

    assert_spans_include_style(&setext, tag("markup.heading"));
    assert_spans_include_style(&reference_link, tag("markup.link"));
    assert_spans_include_style(&reference_definition, tag("markup.link"));
    assert_spans_include_style(&autolink, tag("markup.link"));
    assert_spans_include_style(&underscore_line, tag("markup.emphasis"));
    assert_spans_include_style(&underscore_line, tag("markup.strong"));
    assert_spans_include_style(&indented_code, tag("markup.code"));
    assert_spans_include_style(&tilde_fence, tag("markup.code"));
    assert_spans_include_style(&tilde_body, tag("function"));
}

#[test]
fn test_shell_fixture_highlights_substitutions_and_heredoc_marker() {
    let fixture = include_str!("tests/syntax/fixtures/shell.sh");
    let mut buf = fixture_buffer("syntax-shell-extended", "sh", fixture);

    let parameter = buf
        .syntax_spans_for_line(21)
        .expect("parameter expansion line should exist");
    let command = buf
        .syntax_spans_for_line(22)
        .expect("command substitution line should exist");
    let arithmetic = buf
        .syntax_spans_for_line(23)
        .expect("arithmetic substitution line should exist");
    let heredoc = buf
        .syntax_spans_for_line(26)
        .expect("heredoc opener line should exist");

    assert_spans_include_style(&parameter, tag("string"));
    assert_spans_include_style(&parameter, tag("variable"));
    assert_spans_include_style(&command, tag("string"));
    assert_spans_include_style(&command, tag("punctuation"));
    assert_spans_include_style(&arithmetic, tag("string"));
    assert_spans_include_style(&arithmetic, tag("punctuation"));
    assert_spans_include_style(&heredoc, tag("string.escape"));
}

#[test]
fn test_markdown_prose_does_not_use_generic_identifier_heuristics() {
    let mut buf = syntax_buffer(
        "syntax-prose",
        "md",
        "Capitalized SCREAMY_CASE words stay plain",
    );

    let spans = line_spans(&mut buf, 0);
    assert!(spans.is_empty());
}

#[test]
fn test_markdown_fixture_highlights_common_constructs() {
    let fixture = include_str!("tests/syntax/fixtures/markdown.md");
    let mut buf = syntax_buffer("syntax-markdown-common", "md", fixture);

    let heading = line_spans(&mut buf, 0);
    let prose = line_spans(&mut buf, 2);
    let quote = line_spans(&mut buf, 6);
    let list = line_spans(&mut buf, 8);
    let thematic_break = line_spans(&mut buf, 11);
    let plain = line_spans(&mut buf, 28);

    assert_spans_include_style(&heading, tag("markup.heading"));
    assert_spans_include_style(&prose, tag("markup.emphasis"));
    assert_spans_include_style(&prose, tag("markup.strong"));
    assert_spans_include_style(&prose, tag("markup.code.inline"));
    assert_spans_include_style(&prose, tag("markup.link"));
    assert!(
        prose
            .iter()
            .any(|span| span.style == tag("markup.emphasis.text"))
    );
    assert!(
        prose
            .iter()
            .any(|span| span.style == tag("markup.strong.text"))
    );
    assert!(
        prose
            .iter()
            .any(|span| span.style == tag("markup.code.inline.text"))
    );
    assert_spans_include_style(&quote, tag("markup.quote"));
    assert_spans_include_style(&list, tag("markup.list"));
    assert_spans_include_style(&thematic_break, tag("markup.thematic_break"));
    assert!(plain.is_empty());
}

#[test]
fn test_markdown_code_fence_unknown_capture_is_unstyled() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-fence-unknown", "md").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("```wat\nconst value = 1;\n```", path);

    let body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    assert!(body.is_empty());
    let closing = buf
        .syntax_spans_for_line(2)
        .expect("closing line should exist");
    assert!(closing.iter().any(|span| span.style == tag("markup.code")));
}

#[test]
fn test_javascript_template_string_highlights_interpolation_body() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-template", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("const msg = `hi ${1 + 2} there`;", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("number"));
    assert_spans_include_style(&spans, tag("operator"));
}

#[test]
fn test_javascript_escape_sequences_use_escape_regions() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-escape", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("const msg = \"line 1\\nline 2\";", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
}

#[test]
fn test_javascript_constants_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-constant", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("const value = null;", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("keyword"));
    assert_spans_include_style(&spans, tag("constant"));
}

#[test]
fn test_python_fstring_highlights_interpolation_body() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-py-fstring", "py").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("msg = f\"hello {1 + 2}\"", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("number"));
    assert_spans_include_style(&spans, tag("operator"));
}

#[test]
fn test_python_constants_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-py-constant", "py").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("value = True\nmissing = None", path);

    let first_line = buf
        .syntax_spans_for_line(0)
        .expect("first line should exist");
    let second_line = buf
        .syntax_spans_for_line(1)
        .expect("second line should exist");
    assert_spans_include_style(&first_line, tag("constant"));
    assert_spans_include_style(&second_line, tag("constant"));
}

#[test]
fn test_python_types_use_type_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-py-type", "py").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("class Thing(Exception):\n    pass", path);

    let first_line = buf
        .syntax_spans_for_line(0)
        .expect("first line should exist");
    assert_spans_include_style(&first_line, tag("keyword"));
    assert_spans_include_style(&first_line, tag("type"));
}

#[test]
fn test_json_strings_use_escape_regions() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-json-string", "json").as_path())
        .unwrap();
    let mut buf =
        Buffer::from_str_with_path("{\"key\": \"line 1\\nline 2\", \"enabled\": true}", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("constant"));
}

#[test]
fn test_toml_constants_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-toml-constant", "toml").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("flag = true", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("constant"));
}

#[test]
fn test_toml_datetimes_use_number_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-toml-datetime", "toml").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path(
        "offset = 1979-05-27T07:32:00Z\nlocal = 1979-05-27 07:32:00\ndate = 1979-05-27\ntime = 07:32:00",
        path,
    );

    for line in 0..4 {
        let spans = buf.syntax_spans_for_line(line).expect("line should exist");
        assert_spans_include_style(&spans, tag("number"));
    }
}

#[test]
fn test_toml_literal_strings_remain_plain() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-toml-literal", "toml").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("raw = 'line \\n two'", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert!(!spans.iter().any(|span| span.style == tag("punctuation")));
}

#[test]
fn test_rust_strings_use_escape_regions() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-string", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("let msg = \"hello\\nworld\";", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
}

#[test]
fn test_rust_format_macro_highlights_context_sensitive_format_string() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-format", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("let msg = format!(\"hello {name}\");", path);

    let line = buf.line_at(0).expect("line should exist").to_string();
    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function.macro"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("variable"));
    assert!(spans.iter().any(|span| span.style == tag("string")
        && line[span.start_byte..span.end_byte].contains("hello ")));
}

#[test]
fn test_rust_fixture_format_strings_follow_std_fmt_rules() {
    let fixture = include_str!("tests/syntax/fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-fixture-fmt", "rs", fixture);

    let positional = buf
        .syntax_spans_for_line(24)
        .expect("positional format line should exist");
    let specifier = buf
        .syntax_spans_for_line(25)
        .expect("specifier format line should exist");
    let escaped = buf
        .syntax_spans_for_line(26)
        .expect("escaped format line should exist");
    let escaped_line = buf
        .line_at(26)
        .expect("escaped format line should exist")
        .to_text();
    let escaped_body_start = escaped_line.find('"').expect("opening quote should exist") + 1;
    let escaped_body_end = escaped_line.rfind('"').expect("closing quote should exist");
    let escaped_body = escaped
        .iter()
        .filter(|span| span.start_byte >= escaped_body_start && span.end_byte <= escaped_body_end)
        .collect::<Vec<_>>();

    assert_spans_include_style(&positional, tag("function.macro"));
    assert_spans_include_style(&positional, tag("string"));
    assert_spans_include_style(&positional, tag("punctuation"));
    assert_spans_include_style(&positional, tag("variable"));

    assert_spans_include_style(&specifier, tag("function.macro"));
    assert_spans_include_style(&specifier, tag("string"));
    assert_spans_include_style(&specifier, tag("punctuation"));
    assert_spans_include_style(&specifier, tag("variable"));
    assert_spans_include_style(&specifier, tag("number"));

    assert_spans_include_style(&escaped, tag("function.macro"));
    assert_spans_include_style(&escaped, tag("string"));
    assert_spans_include_style(&escaped, tag("string.escape"));
    assert!(
        !escaped_body
            .iter()
            .any(|span| span.style == tag("variable"))
    );
    assert!(!escaped_body.iter().any(|span| span.style == tag("number")));
}

#[test]
fn test_rust_non_format_string_remains_plain() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-plain-string", "rs").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("\"hello {name}\"", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert!(!spans.iter().any(|span| span.style == tag("function.macro")));
    assert!(!spans.iter().any(|span| span.style == tag("variable")));
    assert!(!spans.iter().any(|span| span.style == tag("punctuation")));
}

#[test]
fn test_rust_format_macro_highlighting_updates_after_edit() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-format-edit", "rs").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("format!(\"hello {name}\")", path);

    assert_spans_include_style(
        &buf.syntax_spans_for_line(0).expect("line should exist"),
        tag("function.macro"),
    );

    buf.insert_text(Cursor::new(0, 0), "let msg = ");

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function.macro"));
    assert_spans_include_style(&spans, tag("string"));
}

#[test]
fn test_shell_single_quotes_remain_plain() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-shell-single", "sh").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("msg='line \\n two'", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert!(!spans.iter().any(|span| span.style == tag("punctuation")));
}

#[test]
fn test_shell_constants_use_constant_rules() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-shell-constant", "sh").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("if true; then echo ok; fi", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("constant"));
}

#[test]
fn test_shell_types_use_type_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-shell-type", "sh").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("local name=\"Ada\"; export PATH=/usr/bin", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("type"));
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
    assert_eq!(buf.test_line_str(0).as_deref(), Some("a"));
    assert_eq!(buf.test_line_str(1).as_deref(), Some(""));
    assert_eq!(buf.test_line_str(2).as_deref(), Some("b"));
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
fn test_inferred_tab_insertion_uses_first_clear_indent_style() {
    let tabs = Buffer::from_str("fn main() {\n\tprintln!(\"hi\");\n}");
    let spaces = Buffer::from_str("fn main() {\n    println!(\"hi\");\n}");
    let mixed = Buffer::from_str("fn main() {\n  \tprintln!(\"hi\");\n\tprintln!(\"bye\");\n}");

    assert_eq!(
        tabs.inferred_tab_insertion(),
        Some(crate::config::TabInsertion::Tabs)
    );
    assert_eq!(
        spaces.inferred_tab_insertion(),
        Some(crate::config::TabInsertion::Spaces)
    );
    assert_eq!(
        mixed.inferred_tab_insertion(),
        Some(crate::config::TabInsertion::Tabs)
    );
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
fn test_tab_display_width_and_expansion() {
    let _config_guard = tab_config_4();

    assert_eq!(display_char_width('\t', 0, 4), 4);
    assert_eq!(display_char_width('\t', 1, 4), 4);
    assert_eq!(display_width_at("a\tb", 0, 4), 6);
    assert_eq!(expand_tabs("a\tb", 0, 4), "a    b");
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

#[test]
fn test_sync_cursor_clamps_to_valid_grapheme_boundary() {
    let buf = Buffer::from_str("a😀b");

    assert_eq!(buf.sync_cursor(Cursor::new(0, 2)), Cursor::new(0, 1));
    assert_eq!(buf.sync_cursor(Cursor::new(0, 4)), Cursor::new(0, 5));
}

#[test]
fn test_sync_cursor_clamps_line_and_column_bounds() {
    let buf = Buffer::from_str("hello\nworld");

    assert_eq!(buf.sync_cursor(Cursor::new(3, 99)), Cursor::new(1, 5));
    assert_eq!(buf.sync_cursor(Cursor::new(0, 99)), Cursor::new(0, 5));
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

assert_next_boundary!(
    test_word_forward_wrap_no_leading_whitespace,
    "hello\nworld",
    Cursor::new(0, 4),
    Boundary::Word,
    Some(Cursor::new(1, 0))
);

assert_next_boundary!(
    test_word_forward_at_word_end,
    "hello world",
    Cursor::new(0, 0),
    Boundary::Word,
    Some(Cursor::new(0, 6))
);

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

assert_next_boundary!(
    test_word_end_at_nonword_sequence_end,
    "hello---world",
    Cursor::new(0, 7),
    Boundary::WordEnd,
    Some(Cursor::new(0, 12)),
    "e should go to end of 'world'"
);

assert_next_boundary!(
    test_word_end_at_word_start,
    "hello world",
    Cursor::new(0, 0),
    Boundary::WordEnd,
    Some(Cursor::new(0, 4))
);

assert_next_boundary!(
    test_word_end_at_word_end,
    "hello world",
    Cursor::new(0, 4),
    Boundary::WordEnd,
    Some(Cursor::new(0, 10))
);

assert_next_boundary!(
    test_word_end_at_last_char_wraps,
    "hello world\nfoo",
    Cursor::new(0, 10),
    Boundary::WordEnd,
    Some(Cursor::new(1, 2))
);

assert_next_boundary!(
    test_bigword_forward_wrap_no_leading_whitespace,
    "hello\nworld",
    Cursor::new(0, 4),
    Boundary::BigWord,
    Some(Cursor::new(1, 0))
);

assert_next_boundary!(
    test_bigword_forward_wrap_with_leading_whitespace,
    "hello\n  world",
    Cursor::new(0, 4),
    Boundary::BigWord,
    Some(Cursor::new(1, 2))
);

// Non-word boundary tests (bug fix for "hello---world" case)

assert_next_boundary!(
    test_word_forward_with_nonword_chars,
    "hello---world",
    Cursor::new(0, 0),
    Boundary::Word,
    Some(Cursor::new(0, 5))
);

assert_next_boundary!(
    test_word_forward_at_nonword_boundary,
    "hello---world",
    Cursor::new(0, 5),
    Boundary::Word,
    Some(Cursor::new(0, 8))
);

assert_next_boundary!(
    test_word_forward_multiple_nonword_chars,
    "a...b",
    Cursor::new(0, 0),
    Boundary::Word,
    Some(Cursor::new(0, 1))
);

assert_next_boundary!(
    test_word_forward_to_spaced_nonword_at_line_end,
    "hello   ---\nworld",
    Cursor::new(0, 0),
    Boundary::Word,
    Some(Cursor::new(0, 8))
);

assert_next_boundary!(
    test_word_forward_to_spaced_nonword_before_next_word,
    "hello   ...   world",
    Cursor::new(0, 0),
    Boundary::Word,
    Some(Cursor::new(0, 8))
);

assert_next_boundary!(
    test_word_forward_from_spaced_nonword_to_next_word,
    "hello   ...   world",
    Cursor::new(0, 8),
    Boundary::Word,
    Some(Cursor::new(0, 14))
);

assert_next_boundary!(
    test_word_forward_nonword_at_start,
    "...hello",
    Cursor::new(0, 0),
    Boundary::Word,
    Some(Cursor::new(0, 3))
);

assert_next_boundary!(
    test_word_end_with_nonword_chars,
    "hello---world",
    Cursor::new(0, 0),
    Boundary::WordEnd,
    Some(Cursor::new(0, 4))
);

assert_next_boundary!(
    test_word_end_at_nonword_boundary,
    "hello---world",
    Cursor::new(0, 5),
    Boundary::WordEnd,
    Some(Cursor::new(0, 7))
);

assert_prev_boundary!(
    test_word_backward_with_nonword_chars,
    "hello---world",
    Cursor::new(0, 11),
    Boundary::Word,
    Some(Cursor::new(0, 8))
);

assert_prev_boundary!(
    test_word_backward_at_nonword_boundary,
    "hello---world",
    Cursor::new(0, 5),
    Boundary::Word,
    Some(Cursor::new(0, 0))
);

assert_prev_boundary!(
    test_word_backward_at_word_boundary_after_nonword,
    "hello---world",
    Cursor::new(0, 8),
    Boundary::Word,
    Some(Cursor::new(0, 5))
);

// BigWordEnd wrap test (bug fix for E key at end of line)

assert_next_boundary!(
    test_bigword_end_at_end_of_word_wraps_to_next_line,
    "hello\nworld",
    Cursor::new(0, 4),
    Boundary::BigWordEnd,
    Some(Cursor::new(1, 4))
);

assert_next_boundary!(
    test_bigword_end_in_middle_of_word,
    "hello world",
    Cursor::new(0, 2),
    Boundary::BigWordEnd,
    Some(Cursor::new(0, 4))
);

assert_next_boundary!(
    test_bigword_end_at_last_char_with_next_word,
    "hello world",
    Cursor::new(0, 4),
    Boundary::BigWordEnd,
    Some(Cursor::new(0, 10))
);

assert_operator_range!(
    test_operator_target_word_forward_range,
    "hello world",
    Cursor::new(0, 0),
    OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    Cursor::new(0, 0),
    Cursor::new(0, 6)
);

assert_operator_range!(
    test_operator_target_word_end_range,
    "hello world",
    Cursor::new(0, 0),
    OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
    Cursor::new(0, 0),
    Cursor::new(0, 5)
);

assert_operator_range!(
    test_operator_target_word_backward_range,
    "hello world",
    Cursor::new(0, 6),
    OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
    Cursor::new(0, 0),
    Cursor::new(0, 6)
);

assert_operator_range!(
    test_operator_target_character_scan_find_forward_range,
    "foo:bar",
    Cursor::new(0, 0),
    OperatorTarget::CharacterScan(FindState {
        target_char: ':',
        kind: FindKind::Find,
        direction: Direction::Forward,
    }),
    Cursor::new(0, 0),
    Cursor::new(0, 4)
);

assert_operator_range!(
    test_operator_target_character_scan_till_forward_range,
    "foo:bar",
    Cursor::new(0, 0),
    OperatorTarget::CharacterScan(FindState {
        target_char: ':',
        kind: FindKind::Till,
        direction: Direction::Forward,
    }),
    Cursor::new(0, 0),
    Cursor::new(0, 3)
);

assert_operator_range!(
    test_operator_target_character_scan_find_backward_range,
    "abcxdef",
    Cursor::new(0, 6),
    OperatorTarget::CharacterScan(FindState {
        target_char: 'x',
        kind: FindKind::Find,
        direction: Direction::Backward,
    }),
    Cursor::new(0, 3),
    Cursor::new(0, 6)
);

assert_operator_range!(
    test_operator_target_character_scan_till_backward_range,
    "abcxdef",
    Cursor::new(0, 6),
    OperatorTarget::CharacterScan(FindState {
        target_char: 'x',
        kind: FindKind::Till,
        direction: Direction::Backward,
    }),
    Cursor::new(0, 4),
    Cursor::new(0, 6)
);

assert_operator_range_with_count!(
    test_operator_target_character_scan_counted_range,
    "foo:bar:baz",
    Cursor::new(0, 0),
    OperatorTarget::CharacterScan(FindState {
        target_char: ':',
        kind: FindKind::Find,
        direction: Direction::Forward,
    }),
    2,
    Cursor::new(0, 0),
    Cursor::new(0, 8)
);

assert_operator_range!(
    test_operator_target_bigword_forward_range,
    "alpha --- beta",
    Cursor::new(0, 0),
    OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
    Cursor::new(0, 0),
    Cursor::new(0, 6)
);

assert_operator_range!(
    test_operator_target_bigword_backward_range,
    "alpha --- beta",
    Cursor::new(0, 10),
    OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
    Cursor::new(0, 6),
    Cursor::new(0, 10)
);

assert_operator_range!(
    test_operator_target_inner_bigword_range,
    "foo-bar baz",
    Cursor::new(0, 2),
    OperatorTarget::TextObject(TextObject::InnerBigWord),
    Cursor::new(0, 0),
    Cursor::new(0, 7)
);

assert_operator_range!(
    test_operator_target_around_bigword_range,
    "foo-bar   baz",
    Cursor::new(0, 2),
    OperatorTarget::TextObject(TextObject::AroundBigWord),
    Cursor::new(0, 0),
    Cursor::new(0, 10)
);

assert_operator_range!(
    test_operator_target_inner_bigword_whitespace_range,
    "x foo-bar baz",
    Cursor::new(0, 1),
    OperatorTarget::TextObject(TextObject::InnerBigWord),
    Cursor::new(0, 1),
    Cursor::new(0, 2)
);

assert_operator_range!(
    test_operator_target_around_bigword_whitespace_range,
    "x foo-bar baz",
    Cursor::new(0, 1),
    OperatorTarget::TextObject(TextObject::AroundBigWord),
    Cursor::new(0, 1),
    Cursor::new(0, 9)
);

assert_operator_range!(
    test_operator_target_bigword_whitespace_only_range,
    "   ",
    Cursor::new(0, 0),
    OperatorTarget::TextObject(TextObject::InnerBigWord),
    Cursor::new(0, 0),
    Cursor::new(0, 3)
);

assert_operator_range!(
    test_operator_target_bigword_empty_line_is_zero_length,
    "",
    Cursor::new(0, 0),
    OperatorTarget::TextObject(TextObject::InnerBigWord),
    Cursor::new(0, 0),
    Cursor::new(0, 0)
);

assert_operator_range!(
    test_operator_target_line_end_range,
    "hello world",
    Cursor::new(0, 6),
    OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
    Cursor::new(0, 6),
    Cursor::new(0, 11)
);

assert_operator_range!(
    test_operator_target_line_start_range,
    "hello world",
    Cursor::new(0, 6),
    OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
    Cursor::new(0, 0),
    Cursor::new(0, 6)
);

assert_operator_range!(
    test_operator_target_line_content_start_range,
    "    hello world",
    Cursor::new(0, 10),
    OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
    Cursor::new(0, 4),
    Cursor::new(0, 10)
);

assert_operator_range_with_count!(
    test_operator_target_counted_word_forward_range,
    "one two three four",
    Cursor::new(0, 0),
    OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    2,
    Cursor::new(0, 0),
    Cursor::new(0, 8)
);

assert_operator_range_with_count!(
    test_operator_target_counted_word_end_range,
    "one two three four",
    Cursor::new(0, 0),
    OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
    2,
    Cursor::new(0, 0),
    Cursor::new(0, 7)
);

assert_operator_range_with_count!(
    test_operator_target_counted_word_backward_range,
    "one two three four",
    Cursor::new(0, 8),
    OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
    2,
    Cursor::new(0, 0),
    Cursor::new(0, 8)
);

assert_linewise_range!(
    test_linewise_operator_target_first_line_range,
    "a\nb\nc\nd\ne",
    Cursor::new(3, 0),
    LinewiseMotion::FirstLine,
    LinewiseDeleteRange::new(0, 4)
);

assert_linewise_range!(
    test_linewise_operator_target_last_line_range,
    "a\nb\nc\nd\ne",
    Cursor::new(1, 0),
    LinewiseMotion::LastLine,
    LinewiseDeleteRange::new(1, 4)
);

assert_linewise_range_with_count!(
    test_linewise_operator_target_counted_first_line_range,
    "a\nb\nc\nd\ne\nf\ng\nh\ni\nj",
    Cursor::new(7, 0),
    LinewiseMotion::FirstLine,
    5,
    LinewiseDeleteRange::new(4, 4)
);

assert_linewise_range_with_count!(
    test_linewise_operator_target_counted_last_line_range,
    "a\nb\nc\nd\ne\nf\ng\nh\ni\nj",
    Cursor::new(2, 0),
    LinewiseMotion::LastLine,
    5,
    LinewiseDeleteRange::new(2, 3)
);

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
    assert!(
        buf.get_operator_target_range_with_count(
            Cursor::new(0, 0),
            OperatorTarget::TextObject(TextObject::InnerBigWord),
            0,
        )
        .is_none()
    );
}

assert_operator_range_with_count!(
    test_operator_target_counted_bigword_range,
    "foo-bar baz qux",
    Cursor::new(0, 0),
    OperatorTarget::TextObject(TextObject::InnerBigWord),
    2,
    Cursor::new(0, 0),
    Cursor::new(0, 11)
);

assert_operator_range!(
    test_operator_target_inner_bracket_range,
    "foo(bar)baz",
    Cursor::new(0, 4),
    OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
    Cursor::new(0, 4),
    Cursor::new(0, 7)
);

assert_operator_range!(
    test_operator_target_around_bracket_range,
    "foo(bar)baz",
    Cursor::new(0, 4),
    OperatorTarget::TextObject(TextObject::AroundBracket(BracketKind::Paren)),
    Cursor::new(0, 3),
    Cursor::new(0, 8)
);

assert_operator_range!(
    test_operator_target_bracket_range_uses_next_pair_on_current_line,
    "x foo(bar) baz",
    Cursor::new(0, 0),
    OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
    Cursor::new(0, 6),
    Cursor::new(0, 9)
);

assert_operator_range_with_count!(
    test_operator_target_bracket_range_nested_count_expands_outward,
    "((foo))",
    Cursor::new(0, 2),
    OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
    2,
    Cursor::new(0, 1),
    Cursor::new(0, 6)
);

assert_operator_range!(
    test_operator_target_bracket_range_multiline,
    "foo(\nbar\n)baz",
    Cursor::new(1, 1),
    OperatorTarget::TextObject(TextObject::AroundBracket(BracketKind::Paren)),
    Cursor::new(0, 3),
    Cursor::new(2, 1)
);

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

assert_operator_range!(
    test_operator_target_inner_bracket_empty_pair_is_zero_length,
    "()",
    Cursor::new(0, 0),
    OperatorTarget::TextObject(TextObject::InnerBracket(BracketKind::Paren)),
    Cursor::new(0, 1),
    Cursor::new(0, 1)
);

assert_operator_range!(
    test_operator_target_inner_quote_range,
    "foo \"bar\" baz",
    Cursor::new(0, 5),
    OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
    Cursor::new(0, 5),
    Cursor::new(0, 8)
);

assert_operator_range!(
    test_operator_target_around_quote_range,
    "foo 'bar' baz",
    Cursor::new(0, 5),
    OperatorTarget::TextObject(TextObject::AroundQuote(QuoteKind::Single)),
    Cursor::new(0, 4),
    Cursor::new(0, 9)
);

assert_operator_range!(
    test_operator_target_quote_range_uses_next_pair_on_current_line,
    "x \"foo\" bar",
    Cursor::new(0, 0),
    OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
    Cursor::new(0, 3),
    Cursor::new(0, 6)
);

assert_operator_range!(
    test_operator_target_quote_range_ignores_escaped_delimiters,
    "foo \"say \\\"hi\\\"\" baz",
    Cursor::new(0, 6),
    OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
    Cursor::new(0, 5),
    Cursor::new(0, 15)
);

assert_operator_range!(
    test_operator_target_quote_range_multiline,
    "\"foo\nbar\nbaz\"",
    Cursor::new(1, 1),
    OperatorTarget::TextObject(TextObject::AroundQuote(QuoteKind::Double)),
    Cursor::new(0, 0),
    Cursor::new(2, 4)
);

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

assert_operator_range!(
    test_operator_target_inner_quote_empty_pair_is_zero_length,
    "\"\"",
    Cursor::new(0, 0),
    OperatorTarget::TextObject(TextObject::InnerQuote(QuoteKind::Double)),
    Cursor::new(0, 1),
    Cursor::new(0, 1)
);

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
    assert_buffer_eq(&buf, "abcd", 1);
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
    assert_buffer_eq(&buf, "line2\nline3", 2);
}

#[test]
fn test_delete_lines_multiple_lines() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
    let cursor = buf.delete_lines(0, 2);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_buffer_eq(&buf, "line3\nline4", 2);
}

#[test]
fn test_delete_lines_from_middle() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4\nline5");
    let cursor = buf.delete_lines(1, 2);
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_buffer_eq(&buf, "line1\nline4\nline5", 3);
}

#[test]
fn test_delete_lines_from_last_line() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.delete_lines(2, 1);
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_buffer_eq(&buf, "line1\nline2", 2);
}

#[test]
fn test_delete_lines_only_line() {
    let mut buf = Buffer::from_str("only line");
    let cursor = buf.delete_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_buffer_eq(&buf, "", 1);
}

#[test]
fn test_delete_lines_count_exceeds_available() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.delete_lines(1, 10); // Only 2 lines from index 1
    assert_eq!(cursor, Some(Cursor::new(0, 0))); // Only line1 remains, at index 0
    assert_buffer_eq(&buf, "line1", 1);
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
    assert_buffer_eq(&buf, "", 1);
}

#[test]
fn test_change_lines_single_line() {
    let mut buf = Buffer::from_str("hello\nworld\ntest");
    let cursor = buf.change_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_buffer_eq(&buf, "\nworld\ntest", 3);
}

#[test]
fn test_change_lines_multiple_lines() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
    let cursor = buf.change_lines(0, 2); // Change 2 lines, leave 1 blank
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_buffer_eq(&buf, "\nline3\nline4", 3);
}

#[test]
fn test_change_lines_from_middle() {
    let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
    let cursor = buf.change_lines(1, 1); // Change line 2
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_buffer_eq(&buf, "line1\n\nline3\nline4", 4);
}

#[test]
fn test_change_lines_from_last_line() {
    let mut buf = Buffer::from_str("line1\nline2");
    let cursor = buf.change_lines(1, 1); // Change last line
    assert_eq!(cursor, Some(Cursor::new(1, 0)));
    assert_buffer_eq(&buf, "line1\n", 2);
}

#[test]
fn test_change_lines_only_line() {
    let mut buf = Buffer::from_str("only line");
    let cursor = buf.change_lines(0, 1);
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_buffer_eq(&buf, "", 1);
}

#[test]
fn test_change_lines_count_exceeds_available() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let cursor = buf.change_lines(0, 5); // Try to change 5 lines, only 3 exist
    assert_eq!(cursor, Some(Cursor::new(0, 0)));
    assert_buffer_eq(&buf, "", 1);
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
    assert_buffer_eq(&buf, "", 1);
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
fn test_insert_lines_after_shifts_ghost_texts_below_insert() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let ghost = buf.insert_ghost_text(Cursor::new(1, 2), Gravity::Right, "g");

    buf.insert_lines_after(0, 1);

    assert_eq!(
        buf.ghost_text(ghost).unwrap().kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(2, 2),
            gravity: Gravity::Right,
        })
    );
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
fn test_insert_lines_before_shifts_ghost_texts_at_and_below_insert() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let ghost = buf.insert_ghost_text(Cursor::new(1, 2), Gravity::Right, "g");

    buf.insert_lines_before(1, 1);

    assert_eq!(
        buf.ghost_text(ghost).unwrap().kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(2, 2),
            gravity: Gravity::Right,
        })
    );
}

#[test]
fn test_paste_linewise_content_shifts_ghost_texts() {
    let mut buf = Buffer::from_str("line1\nline2\nline3");
    let ghost = buf.insert_ghost_text(Cursor::new(1, 2), Gravity::Right, "g");
    let pasted = vec![Arc::from("a"), Arc::from("b")];

    buf.paste_linewise_content(0, &pasted, true)
        .expect("paste should succeed");

    assert_eq!(
        buf.ghost_text(ghost).unwrap().kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(3, 2),
            gravity: Gravity::Right,
        })
    );
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

#[test]
fn test_inferred_auto_indent_prefix_prefers_more_indented_neighbor() {
    let buf = Buffer::from_str("  first\n\n    second");

    assert_eq!(
        buf.inferred_auto_indent_prefix(Cursor::new(1, 0)),
        Some("    ".to_string())
    );
}

#[test]
fn test_inferred_auto_indent_prefix_preserves_exact_prefix() {
    let buf = Buffer::from_str("\t  first\n  second");

    assert_eq!(
        buf.inferred_auto_indent_prefix(Cursor::new(0, 0)),
        Some("\t  ".to_string())
    );
}

#[test]
fn test_inferred_auto_indent_prefix_ignores_blank_lines() {
    let buf = Buffer::from_str("first\n\nsecond");

    assert_eq!(buf.inferred_auto_indent_prefix(Cursor::new(1, 0)), None);
}

#[test]
fn test_shift_line_indentation_increases_and_decreases_with_spaces() {
    let _guard = tab_config_4();
    let mut buf = Buffer::from_str("hello");

    assert_eq!(buf.increase_line_indentation(0), Some(4));
    assert_eq!(buf.as_str(), "    hello");
    assert_eq!(buf.decrease_line_indentation(0), Some(4));
    assert_eq!(buf.as_str(), "hello");
}

#[test]
fn test_shift_line_indentation_uses_tabs_when_buffer_style_is_tabs() {
    let _guard = tab_config_4();
    let mut buf = Buffer::from_str("fn main() {\n\t\tprintln!(\"hi\");");

    assert_eq!(buf.decrease_line_indentation(1), Some(1));
    assert_eq!(buf.as_str(), "fn main() {\n\tprintln!(\"hi\");");
}

#[test]
fn test_shift_line_indentation_preserves_mixed_remaining_prefix() {
    let _guard = tab_config_4();
    let mut buf = Buffer::from_str("\t  hello");

    assert_eq!(buf.decrease_line_indentation(0), Some(1));
    assert_eq!(buf.as_str(), "  hello");
}

#[test]
fn test_shift_line_indentation_stops_at_column_zero() {
    let _guard = tab_config_4();
    let mut buf = Buffer::from_str("  hello");

    assert_eq!(buf.decrease_line_indentation(0), Some(2));
    assert_eq!(buf.as_str(), "hello");
    assert_eq!(buf.decrease_line_indentation(0), Some(0));
    assert_eq!(buf.as_str(), "hello");
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
    let buf = three_para_buffer();

    // From middle of Para 2 (line 2), should find blank line before it (line 1)
    let cursor = Cursor::new(2, 5);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From line 3 (Para 2 line 2), should find blank line before Para 2 (line 1)
    let cursor = Cursor::new(3, 5);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From Para 1 (line 0), should clamp to BOF instead of stopping early.
    let cursor = Cursor::new(0, 5);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(0, 0)));
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
    let buf = three_para_buffer();

    // From blank line 1, should clamp to BOF instead of stopping early.
    let cursor = Cursor::new(1, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(0, 0)));

    // From blank line 4, should find blank line before Para 2 (line 1).
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

    // From Para 1 (line 0), clamp to BOF instead of stopping early.
    let cursor = Cursor::new(0, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(0, 0)));
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
    let buf = three_para_buffer();

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
    let buf = three_para_buffer();

    // From blank line 1, should find blank line after Para 2 (line 4).
    let cursor = Cursor::new(1, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(4, 0)));

    // From blank line 4, should clamp to EOF instead of stopping early.
    let cursor = Cursor::new(4, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(5, 13)));
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

    // From Para 1 (line 0), one paragraph forward.
    let cursor = Cursor::new(0, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(1, 0)));

    // From Para 2 (line 2), one paragraph forward.
    let cursor = Cursor::new(2, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(3, 0)));

    // From Para 3 (line 4), clamp to EOF instead of stopping early.
    let cursor = Cursor::new(4, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(4, 6)));
}

#[test]
fn test_cursor_paragraph_forward_clamps_to_eof() {
    // Buffer ends with two blank lines, which should be skipped all the way to EOF.
    let buf = Buffer::from_str("Para 1\n\nPara 2\n \n ");

    let cursor = Cursor::new(2, 0);
    let result = buf.cursor_paragraph_forward(cursor);
    assert_eq!(result, Some(Cursor::new(4, 1)));
}

#[test]
fn test_cursor_paragraph_backward_clamps_to_bof() {
    // Buffer begins with two blank lines, which should be skipped all the way to BOF.
    let buf = Buffer::from_str(" \n \nPara 1\n\nPara 2");

    let cursor = Cursor::new(2, 0);
    let result = buf.cursor_paragraph_backward(cursor);
    assert_eq!(result, Some(Cursor::new(0, 0)));
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
    assert_eq!(
        buf.cursor_paragraph_backward(cursor),
        Some(Cursor::new(0, 0))
    );
    assert_eq!(
        buf.cursor_paragraph_forward(cursor),
        Some(Cursor::new(0, 11))
    );
}

#[test]
fn test_apply_completion_applies_additional_edits() {
    let mut buf = Buffer::from_str("abcdef");
    let range = TextObjectRange {
        start: Cursor::new(0, 2),
        end: Cursor::new(0, 4),
    };
    let edits = vec![crate::ui::completion::CompletionTextEdit {
        range: TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 2),
        },
        text: "Z".to_string(),
    }];

    let cursor = buf
        .apply_completion(range, "X", 1, edits.as_slice())
        .expect("apply completion edits");

    assert_eq!(buf.as_str(), "ZXef");
    assert_eq!(cursor, Cursor::new(0, 2));
}

#[test]
fn test_apply_completion_tracks_cursor_after_multiline_additional_edits() {
    let mut buf = Buffer::from_str("abcdef");
    let range = TextObjectRange {
        start: Cursor::new(0, 2),
        end: Cursor::new(0, 4),
    };
    let edits = vec![crate::ui::completion::CompletionTextEdit {
        range: TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 2),
        },
        text: "Z\nY".to_string(),
    }];

    let cursor = buf
        .apply_completion(range, "X", 1, edits.as_slice())
        .expect("apply completion edits");

    assert_eq!(buf.as_str(), "Z\nYXef");
    assert_eq!(cursor, Cursor::new(1, 2));
}
