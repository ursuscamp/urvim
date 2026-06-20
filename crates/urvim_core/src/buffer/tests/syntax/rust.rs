use super::*;
use crate::buffer::SyntaxFoldRegion;

fn rust_fixture_source() -> &'static str {
    include_str!("../../../../../urvim_syntax/fixtures/rust.rs")
}

fn assert_fold_regions(buf: &mut Buffer, expected: &[(usize, usize)]) {
    let regions: Vec<SyntaxFoldRegion> = buf.syntax_fold_regions().to_vec();
    let actual: Vec<(usize, usize)> = regions
        .iter()
        .map(|region| (region.start_line, region.end_line))
        .collect();
    assert_eq!(
        actual, expected,
        "expected fold regions {expected:?}, got {actual:?}"
    );
}

fn line_containing(buf: &Buffer, needle: &str) -> usize {
    (0..buf.line_count())
        .find(|line| {
            buf.line_at(*line)
                .is_some_and(|line_text| line_text.to_string().contains(needle))
        })
        .unwrap_or_else(|| panic!("fixture should contain {needle:?}"))
}

#[test]
fn test_rust_fixture_uses_grammar_rules() {
    let mut buf = fixture_buffer("syntax-rust-fixture", "rs", rust_fixture_source());

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
    let char_line = buf
        .syntax_spans_for_line(6)
        .expect("char line should exist");
    let escaped_char_line = buf
        .syntax_spans_for_line(7)
        .expect("escaped char line should exist");

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
    assert_spans_include_style(&char_line, tag("constant"));
    assert_spans_include_style(&escaped_char_line, tag("constant"));
    assert_exact_spans(
        &buf.syntax_spans_for_line(24)
            .expect("format line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 17, tag("variable")),
            (18, 19, tag("operator")),
            (20, 27, tag("function.macro")),
            (27, 28, tag("punctuation")),
            (28, 29, tag("string")),
            (29, 36, tag("string")),
            (36, 37, tag("punctuation")),
            (37, 38, tag("punctuation")),
            (38, 39, tag("string")),
            (39, 40, tag("string")),
            (40, 41, tag("punctuation")),
            (42, 47, tag("variable")),
            (47, 48, tag("punctuation")),
            (48, 49, tag("punctuation")),
        ],
    );
}

#[test]
fn test_rust_fold_regions_for_multiline_braces() {
    let mut buf = syntax_buffer("rust-fold-braces", "rs", "fn main() {\n    let x = 1;\n}\n");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(0, 2)]);
}

#[test]
fn test_rust_fold_regions_include_parens_and_brackets() {
    let mut buf = syntax_buffer(
        "rust-fold-delimiters",
        "rs",
        "fn main(\n    x: i32,\n) -> [i32;\n    1\n] {\n}\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(0, 2), (2, 4), (4, 5)]);
}

#[test]
fn test_rust_fold_region_starting_at_prefers_longest_same_line_fold() {
    let mut buf = syntax_buffer(
        "rust-fold-same-start-longest",
        "rs",
        "fn main( {\n    arg\n)\n    body();\n}\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(0, 2), (0, 4)]);
    let region = buf
        .syntax_fold_region_starting_at(0)
        .expect("line 0 should start a syntax fold");
    assert_eq!((region.start_line, region.end_line), (0, 4));
}

#[test]
fn test_rust_same_line_folds_are_discarded() {
    let mut buf = syntax_buffer("rust-fold-same-line", "rs", "fn main() { let x = 1; }\n");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[]);
}

#[test]
fn test_rust_unmatched_close_is_ignored() {
    let mut buf = syntax_buffer("rust-fold-unmatched-close", "rs", "}\nfn main() {\n}\n");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(1, 2)]);
}

#[test]
fn test_rust_unclosed_open_folds_to_eof() {
    let mut buf = syntax_buffer("rust-fold-unclosed", "rs", "fn main() {\n    let x = 1;\n");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    // EOF is line 1 because the file lacks a trailing newline.
    assert_fold_regions(&mut buf, &[(0, 1)]);
}

#[test]
fn test_rust_mismatched_delimiter_closes_nearest_matching() {
    let mut buf = syntax_buffer("rust-fold-mismatched", "rs", "foo(\n    {\n)\n}\n");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    // `)` closes `(`, leaving `{` open so `}` can close it later.
    assert_fold_regions(&mut buf, &[(0, 2), (1, 3)]);
}

#[test]
fn test_rust_attribute_brackets_emit_fold_regions() {
    let mut buf = syntax_buffer(
        "rust-fold-attribute",
        "rs",
        "#[derive(\n    Debug,\n)]\nstruct Thing;\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(0, 2)]);
}

#[test]
fn test_rust_format_macro_call_parentheses_emit_fold_region() {
    let mut buf = syntax_buffer(
        "rust-fold-format-macro",
        "rs",
        "fn main() {\n    format!(\n        \"{}\",\n        value,\n    );\n}\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(1, 4), (0, 5)]);
}

#[test]
fn test_rust_fold_regions_rebuild_after_edit_from_cached_prefix() {
    let mut buf = syntax_buffer(
        "rust-fold-edit-rebuild",
        "rs",
        "fn main() {\n    if value {\n        work();\n    }\n}\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    assert_fold_regions(&mut buf, &[(1, 3), (0, 4)]);

    buf.insert_text(Cursor::new(2, 8), "more_");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(1, 3), (0, 4)]);
}

#[test]
fn test_rust_fold_regions_below_inserted_line_are_rebuilt() {
    let mut buf = syntax_buffer(
        "rust-fold-line-insert-rebuild",
        "rs",
        "fn first() {\n}\n\nfn second() {\n    work();\n}\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    assert_fold_regions(&mut buf, &[(0, 1), (3, 5)]);

    buf.insert_lines_before(2, 1);

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    assert_fold_regions(&mut buf, &[(0, 1), (4, 6)]);

    let region = buf
        .syntax_fold_region_starting_at(4)
        .expect("fold below inserted line should be rebuilt");
    assert_eq!((region.start_line, region.end_line), (4, 6));
}

#[test]
fn test_rust_fold_region_after_struct_survives_insert_above_function() {
    let mut buf = syntax_buffer(
        "rust-fold-main-after-struct-insert",
        "rs",
        "#[derive(Parser)]\nstruct Cli {\n    files: Vec<String>,\n}\n\nfn main() {\n    run();\n}\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    assert_fold_regions(&mut buf, &[(1, 3), (5, 7)]);

    buf.insert_lines_before(5, 1);

    assert!(
        buf.cached_syntax_fold_region_starting_at(6).is_some(),
        "cached main fold should survive inserting above it"
    );

    let region = buf
        .syntax_fold_region_starting_at(6)
        .expect("main fold should be rebuilt after inserting above it");
    assert_eq!((region.start_line, region.end_line), (6, 8));
}

#[test]
fn test_rust_delimiters_in_comments_and_strings_do_not_fold() {
    let mut buf = syntax_buffer(
        "rust-fold-no-string-comment",
        "rs",
        "fn main() {\n    let s = \"{\";\n    // {\n}\n",
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    assert_fold_regions(&mut buf, &[(0, 3)]);
}

#[test]
fn test_rust_syntax_supports_folding() {
    let buf = syntax_buffer("rust-fold-support", "rs", "fn main() {}\n");
    assert!(buf.syntax_supports_folding());
}

#[test]
fn test_rust_fixture_highlights_raw_and_format_strings_exactly() {
    let mut buf = fixture_buffer("syntax-rust-exact-spans", "rs", rust_fixture_source());

    assert_exact_spans(
        &buf.syntax_spans_for_line(1)
            .expect("block comment line should exist"),
        &[
            (0, 2, tag("comment.block")),
            (2, 17, tag("comment.block")),
            (17, 19, tag("comment.block")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(24)
            .expect("format line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 17, tag("variable")),
            (18, 19, tag("operator")),
            (20, 27, tag("function.macro")),
            (27, 28, tag("punctuation")),
            (28, 29, tag("string")),
            (29, 36, tag("string")),
            (36, 37, tag("punctuation")),
            (37, 38, tag("punctuation")),
            (38, 39, tag("string")),
            (39, 40, tag("string")),
            (40, 41, tag("punctuation")),
            (42, 47, tag("variable")),
            (47, 48, tag("punctuation")),
            (48, 49, tag("punctuation")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(25)
            .expect("named format line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 23, tag("variable")),
            (24, 25, tag("operator")),
            (26, 33, tag("function.macro")),
            (33, 34, tag("punctuation")),
            (34, 35, tag("string")),
            (35, 36, tag("punctuation")),
            (36, 40, tag("variable.global")),
            (40, 41, tag("punctuation")),
            (41, 43, tag("number")),
            (43, 44, tag("punctuation")),
            (44, 45, tag("string")),
            (45, 46, tag("punctuation")),
            (47, 51, tag("variable")),
            (52, 53, tag("operator")),
            (54, 59, tag("variable")),
            (59, 60, tag("punctuation")),
            (60, 61, tag("punctuation")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(26)
            .expect("escaped format line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 15, tag("variable")),
            (16, 17, tag("operator")),
            (18, 25, tag("function.macro")),
            (25, 26, tag("punctuation")),
            (26, 27, tag("string")),
            (27, 29, tag("string.escape")),
            (29, 36, tag("string")),
            (36, 38, tag("string.escape")),
            (38, 39, tag("string")),
            (39, 40, tag("punctuation")),
            (40, 41, tag("punctuation")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(39)
            .expect("raw string opener line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 11, tag("variable")),
            (12, 13, tag("operator")),
            (14, 17, tag("string")),
            (17, 21, tag("string")),
            (21, 22, tag("string")),
            (22, 28, tag("string")),
            (28, 29, tag("string")),
            (29, 31, tag("string")),
            (31, 32, tag("punctuation")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(40)
            .expect("raw string body line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 21, tag("variable")),
            (22, 23, tag("operator")),
            (24, 27, tag("string")),
            (27, 32, tag("string")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(41)
            .expect("raw string closing line should exist"),
        &[
            (0, 6, tag("string")),
            (6, 8, tag("string")),
            (8, 9, tag("punctuation")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(42)
            .expect("bytes line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 13, tag("variable")),
            (14, 15, tag("operator")),
            (16, 18, tag("string")),
            (18, 21, tag("string")),
            (21, 23, tag("string.escape")),
            (23, 24, tag("string")),
            (24, 25, tag("punctuation")),
        ],
    );
    assert_exact_spans(
        &buf.syntax_spans_for_line(43)
            .expect("raw bytes line should exist"),
        &[
            (4, 7, tag("keyword")),
            (8, 17, tag("variable")),
            (18, 19, tag("operator")),
            (20, 24, tag("string")),
            (24, 33, tag("string")),
            (33, 35, tag("string")),
            (35, 36, tag("punctuation")),
        ],
    );
}

#[test]
fn test_rust_nested_block_comments_are_exactly_spanned() {
    let mut buf = fixture_buffer(
        "syntax-rust-nested-comment",
        "rs",
        "/* outer /* inner */ outer */",
    );

    let spans = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    assert_exact_spans(
        &spans,
        &[
            (0, 2, tag("comment.block")),
            (2, 9, tag("comment.block")),
            (9, 11, tag("comment.block")),
            (11, 18, tag("comment.block")),
            (18, 20, tag("comment.block")),
            (20, 27, tag("comment.block")),
            (27, 29, tag("comment.block")),
        ],
    );
}

#[test]
fn test_rust_rehighlights_after_mid_file_type_edit_and_top_insert() {
    let mut buf = fixture_buffer("syntax-rust-main-edit", "rs", rust_fixture_source());

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    let option_line_idx = line_containing(&buf, "Option<String>");
    let option_line = buf
        .line_at(option_line_idx)
        .expect("theme line should exist")
        .to_string();
    let string_start = option_line
        .find("String")
        .expect("line should contain String");
    buf.remove(
        Cursor::new(option_line_idx, string_start),
        Cursor::new(option_line_idx, string_start + "String".len()),
    );
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    let result_line_idx = line_containing(&buf, "Some(guard)");
    let config_line_text = buf
        .line_at(result_line_idx)
        .expect("config line should exist")
        .to_string();
    let config_line = buf
        .syntax_spans_for_line(result_line_idx)
        .expect("config line should remain highlighted after edit");
    assert_spans_include_exact_style(&config_line, &config_line_text, "Option", tag("type"));

    buf.insert_text(Cursor::new(0, 0), "\n");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    let later_line_idx = line_containing(&buf, "Some(guard)");
    let later_line_text = buf
        .line_at(later_line_idx)
        .expect("later line should exist")
        .to_string();
    let later_line = buf
        .syntax_spans_for_line(later_line_idx)
        .expect("later line should remain highlighted after top insert");
    assert_spans_include_exact_style(&later_line, &later_line_text, "Option", tag("type"));
}

#[test]
fn test_rust_keeps_prefix_highlight_after_insert_inside_main() {
    let mut buf = fixture_buffer("syntax-rust-main-inner-insert", "rs", rust_fixture_source());

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let insert_line = line_containing(&buf, "let guard");
    buf.insert_text(Cursor::new(insert_line, 0), "\n");
    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));

    let use_line_idx = line_containing(&buf, "let value");
    let use_line_text = buf
        .line_at(use_line_idx)
        .expect("value line should exist")
        .to_string();
    let use_line = buf
        .syntax_spans_for_line(use_line_idx)
        .expect("value line should remain highlighted");
    assert_spans_include_exact_style(&use_line, &use_line_text, "let", tag("keyword"));

    let main_line_idx = line_containing(&buf, "fn completion_fixture");
    let main_line_text = buf
        .line_at(main_line_idx)
        .expect("fixture function line should exist")
        .to_string();
    let main_line = buf
        .syntax_spans_for_line(main_line_idx)
        .expect("fixture function line should remain highlighted");
    assert_spans_include_exact_style(&main_line, &main_line_text, "fn", tag("keyword"));
}

#[test]
fn test_rust_cache_retains_prefix_spans_after_insert_after_main_start() {
    let mut buf = fixture_buffer("syntax-rust-main-prefix-cache", "rs", rust_fixture_source());

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let insert_line = line_containing(&buf, "let guard");
    buf.insert_text(Cursor::new(insert_line, 0), "\n");

    let prefix_line_idx = line_containing(&buf, "let value");
    let before = buf
        .cached_syntax_spans_for_line(prefix_line_idx)
        .expect("prefix line should stay cached");
    assert!(!before.is_empty());
    let after = buf
        .cached_syntax_spans_for_line(prefix_line_idx)
        .expect("prefix line should stay cached after insert");
    assert!(!after.is_empty());

    let main_adjacent_line = line_containing(&buf, "let guard");
    let main_line = buf
        .syntax_spans_for_line(main_adjacent_line)
        .expect("new main-adjacent line should still highlight");
    assert!(!main_line.is_empty());
}

#[test]
fn test_rust_keeps_prefix_highlight_after_completion_then_insert_char() {
    let mut buf = fixture_buffer(
        "syntax-rust-completion-then-insert",
        "rs",
        rust_fixture_source(),
    );

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let completion_line = line_containing(&buf, "let guard");
    let line_text = buf
        .line_at(completion_line)
        .expect("guard line should exist")
        .to_string();
    let guard_start = line_text.find("guard").expect("line should contain guard");
    let guard_end = guard_start + "guard".len();
    let range = TextObjectRange {
        start: Cursor::new(completion_line, guard_start),
        end: Cursor::new(completion_line, guard_end),
    };

    let cursor = buf
        .apply_completion(range, "guard_handle", "guard_handle".len(), &[])
        .expect("completion should apply");
    buf.ensure_syntax_through(completion_line);
    buf.insert_char(cursor, '_');

    let use_line_idx = line_containing(&buf, "let value");
    let use_line_text = buf
        .line_at(use_line_idx)
        .expect("value line should exist")
        .to_string();
    let use_line = buf
        .cached_syntax_spans_for_line(use_line_idx)
        .expect("prefix line should remain cached after completion and insert");
    assert_spans_include_exact_style(&use_line, &use_line_text, "let", tag("keyword"));

    let main_line_idx = line_containing(&buf, "fn completion_fixture");
    let main_line_text = buf
        .line_at(main_line_idx)
        .expect("fixture function line should exist")
        .to_string();
    let main_line = buf
        .cached_syntax_spans_for_line(main_line_idx)
        .expect("main line should remain cached after completion and insert");
    assert_spans_include_exact_style(&main_line, &main_line_text, "fn", tag("keyword"));
}

#[test]
fn test_rust_keeps_prefix_highlight_after_lsp_completion_edits_then_insert_char() {
    let mut buf = fixture_buffer(
        "syntax-rust-lsp-completion-then-insert",
        "rs",
        rust_fixture_source(),
    );

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let completion_line = line_containing(&buf, "let guard");
    let line_text = buf
        .line_at(completion_line)
        .expect("guard line should exist")
        .to_string();
    let guard_start = line_text.find("guard").expect("line should contain guard");
    let guard_end = guard_start + "guard".len();
    let range = TextObjectRange {
        start: Cursor::new(completion_line, guard_start),
        end: Cursor::new(completion_line, guard_end),
    };
    let additional_edits = vec![crate::ui::completion::CompletionTextEdit {
        range: TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 0),
        },
        text: "use std::borrow::Cow;\n".to_string(),
    }];

    let cursor = buf
        .apply_completion(
            range,
            "guard_handle",
            "guard_handle".len(),
            &additional_edits,
        )
        .expect("completion should apply");
    buf.insert_char(cursor, '_');

    assert!(
        buf.cached_syntax_spans_for_line(0).is_none(),
        "inserted use line should be explicitly missing before refresh"
    );

    let main_line_idx = line_containing(&buf, "fn completion_fixture");
    let main_line_text = buf
        .line_at(main_line_idx)
        .expect("fixture function line should exist")
        .to_string();
    let main_line = buf
        .syntax_spans_for_line(main_line_idx)
        .expect("main line should highlight on demand after LSP completion and insert");
    assert_spans_include_exact_style(&main_line, &main_line_text, "fn", tag("keyword"));

    let inserted_use_text = buf
        .line_at(0)
        .expect("inserted use should exist")
        .to_string();
    let inserted_use_line = buf
        .cached_syntax_spans_for_line(0)
        .expect("on-demand highlight should fill inserted use line");
    assert_spans_include_exact_style(
        &inserted_use_line,
        &inserted_use_text,
        "use",
        tag("keyword"),
    );
}

#[test]
fn test_rust_lsp_completion_top_edit_follow_up_char_drops_shifted_render_fallback() {
    let body = (0..1024)
        .map(|idx| format!("fn filler_{idx}() {{ let value_{idx} = {idx}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() {{\n    let guard = String::new()\n}}\n{body}");
    let mut buf = fixture_buffer("syntax-rust-lsp-completion-follow-up-render", "rs", &source);

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let completion_line = line_containing(&buf, "let guard");
    let line_text = buf
        .line_at(completion_line)
        .expect("guard line should exist")
        .to_string();
    let guard_start = line_text.find("guard").expect("line should contain guard");
    let guard_end = guard_start + "guard".len();
    let range = TextObjectRange {
        start: Cursor::new(completion_line, guard_start),
        end: Cursor::new(completion_line, guard_end),
    };
    let additional_edits = vec![crate::ui::completion::CompletionTextEdit {
        range: TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 0),
        },
        text: "use std::borrow::Cow;\n".to_string(),
    }];

    let cursor = buf
        .apply_completion(
            range,
            "guard_handle",
            "guard_handle".len(),
            &additional_edits,
        )
        .expect("completion should apply");
    buf.insert_char(cursor, ';');

    let edited_line = line_containing(&buf, "guard_handle");
    assert!(
        buf.cached_syntax_spans_for_line(edited_line).is_none(),
        "follow-up edited line should be missing before refresh"
    );
    let rendered_filler_line = line_containing(&buf, "fn filler_1023()");
    let rendered_line = buf
        .line_at(rendered_filler_line)
        .expect("line should exist");
    assert!(
        buf.render_syntax_spans_for_line_ref(rendered_filler_line, &rendered_line)
            .is_some(),
        "far unchanged matching line can render from its cached entry"
    );
}

#[test]
fn test_rust_lsp_completion_top_edit_does_not_force_full_cache_warm() {
    let body = (0..1024)
        .map(|idx| format!("fn filler_{idx}() {{ let value_{idx} = {idx}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() {{\n    let guard = String::new();\n}}\n{body}");
    let mut buf = fixture_buffer("syntax-rust-lsp-large-completion", "rs", &source);

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let completion_line = line_containing(&buf, "let guard");
    let line_text = buf
        .line_at(completion_line)
        .expect("guard line should exist")
        .to_string();
    let guard_start = line_text.find("guard").expect("line should contain guard");
    let range = TextObjectRange {
        start: Cursor::new(completion_line, guard_start),
        end: Cursor::new(completion_line, guard_start + "guard".len()),
    };
    let additional_edits = vec![crate::ui::completion::CompletionTextEdit {
        range: TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 0),
        },
        text: "use std::borrow::Cow;\n".to_string(),
    }];

    buf.apply_completion(
        range,
        "guard_handle",
        "guard_handle".len(),
        &additional_edits,
    )
    .expect("completion should apply");

    assert!(!buf.syntax_cache_complete());
    assert!(buf.cached_syntax_spans_for_line(0).is_none());
}

#[test]
fn test_rust_lsp_completion_line_shift_has_no_stale_render_fallback() {
    let body = (0..1024)
        .map(|idx| format!("fn filler_{idx}() {{ let value_{idx} = {idx}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() {{\n    let guard = String::new();\n}}\n{body}");
    let mut buf = fixture_buffer("syntax-rust-lsp-render-fallback", "rs", &source);

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let completion_line = line_containing(&buf, "let guard");
    let line_text = buf
        .line_at(completion_line)
        .expect("guard line should exist")
        .to_string();
    let guard_start = line_text.find("guard").expect("line should contain guard");
    let range = TextObjectRange {
        start: Cursor::new(completion_line, guard_start),
        end: Cursor::new(completion_line, guard_start + "guard".len()),
    };
    let additional_edits = vec![crate::ui::completion::CompletionTextEdit {
        range: TextObjectRange {
            start: Cursor::new(0, 0),
            end: Cursor::new(0, 0),
        },
        text: "use std::borrow::Cow;\n".to_string(),
    }];

    buf.apply_completion(
        range,
        "guard_handle",
        "guard_handle".len(),
        &additional_edits,
    )
    .expect("completion should apply");

    let inserted_use_line = buf.line_at(0).expect("inserted use should exist");
    assert!(
        buf.render_syntax_spans_for_line_ref(0, &inserted_use_line)
            .is_none(),
        "inserted lines should not render stale syntax spans before refresh"
    );
}

#[test]
fn test_rust_line_shift_render_has_no_stale_fallback() {
    let body = (0..1024)
        .map(|idx| format!("fn filler_{idx}() {{ let value_{idx} = {idx}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() {{\n    let guard = String::new();\n}}\n{body}");
    let mut buf = fixture_buffer("syntax-rust-render-fallback-mismatch", "rs", &source);

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    buf.insert_text(Cursor::new(0, 0), "use std::borrow::Cow;\n");

    let inserted_use_line = buf.line_at(0).expect("inserted use should exist");
    assert!(
        buf.render_syntax_spans_for_line_ref(0, &inserted_use_line)
            .is_none(),
        "inserted lines should not render stale syntax spans before refresh"
    );
}

#[test]
fn test_rust_same_line_edit_keeps_dirty_viewport_highlighted() {
    let body = (0..1024)
        .map(|idx| format!("fn filler_{idx}() {{ let value_{idx} = {idx}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("fn main() {{\n    let guard = String::new();\n}}\n{body}");
    let mut buf = fixture_buffer("syntax-rust-same-line-edit", "rs", &source);

    buf.ensure_syntax_through(buf.line_count().saturating_sub(1));
    let edit_line = line_containing(&buf, "let guard");
    let line_text = buf
        .line_at(edit_line)
        .expect("guard line should exist")
        .to_string();
    let guard_start = line_text.find("guard").expect("line should contain guard");

    buf.remove(
        Cursor::new(edit_line, guard_start),
        Cursor::new(edit_line, guard_start + "guard".len()),
    );

    assert!(!buf.syntax_cache_complete());

    let rendered_filler_line = line_containing(&buf, "fn filler_1023()");
    let filler_line = buf
        .line_at(rendered_filler_line)
        .expect("filler line should exist");
    let filler_line_text = filler_line.to_string();
    let dirty_spans = buf
        .render_syntax_spans_for_line_ref(rendered_filler_line, &filler_line)
        .expect("rendered line should keep dirty syntax spans");

    assert_spans_include_exact_style(dirty_spans, &filler_line_text, "fn", tag("keyword"));
}

#[test]
fn test_rust_character_literals_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-char", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("let a = 'x'; let b = '\\n'; let c = b'\\t';", path);

    let line = buf.line_at(0).expect("line should exist").to_string();
    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    for literal in ["'x'", "'\\n'", "b'\\t'"] {
        let start = line.find(literal).expect("literal should exist");
        let end = start + literal.len();
        assert!(spans.iter().any(|span| {
            span.style == tag("constant") && span.start_byte <= start && span.end_byte >= end
        }));
    }
}

#[test]
fn test_rust_fixture_highlights_extended_literals() {
    let mut buf = fixture_buffer("syntax-rust-extended", "rs", rust_fixture_source());

    let doc_comment_idx = line_containing(&buf, "/// Doc comment");
    let attribute_idx = line_containing(&buf, "#[inline]");
    let raw_string_idx = line_containing(&buf, "raw = r#");
    let raw_multiline_idx = line_containing(&buf, "let raw_multiline");
    let byte_string_idx = line_containing(&buf, "bytes = b");
    let raw_bytes_idx = line_containing(&buf, "raw_bytes");
    let numeric_idx = line_containing(&buf, "hex = 0xff_u8");
    let namespace_idx = line_containing(&buf, "std::mem::drop");
    let doc_comment = buf
        .syntax_spans_for_line(doc_comment_idx)
        .expect("doc comment line should exist");
    let attribute = buf
        .syntax_spans_for_line(attribute_idx)
        .expect("attribute line should exist");
    let attribute_line = buf
        .line_at(attribute_idx)
        .expect("attribute line should exist")
        .to_string();
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
    let namespace = buf
        .syntax_spans_for_line(namespace_idx)
        .expect("namespace line should exist");

    assert_spans_include_style(&doc_comment, tag("comment.documentation"));
    assert_spans_include_exact_style(
        &attribute,
        attribute_line.as_str(),
        "#[",
        tag("punctuation"),
    );
    assert_spans_include_exact_style(&attribute, attribute_line.as_str(), "]", tag("punctuation"));
    assert_spans_include_style(&raw_string, tag("string"));
    assert_spans_include_style(&raw_multiline, tag("string"));
    assert_spans_include_style(&byte_string, tag("string"));
    assert_spans_include_style(&raw_bytes, tag("string"));
    assert_spans_include_style(&numeric, tag("number"));
    assert_spans_include_style(&namespace, tag("namespace"));
    assert_spans_include_style(&namespace, tag("function"));
    assert!(raw_multiline.iter().any(|span| span.style == tag("string")));
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
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/rust.rs");
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
    let specifier_line = buf
        .line_at(25)
        .expect("specifier format line should exist")
        .to_string();

    assert_spans_include_style(&positional, tag("function.macro"));
    assert_spans_include_style(&positional, tag("string"));
    assert_spans_include_style(&positional, tag("punctuation"));
    assert_spans_include_style(&positional, tag("variable"));

    assert_spans_include_style(&specifier, tag("function.macro"));
    assert_spans_include_style(&specifier, tag("string"));
    assert_spans_include_style(&specifier, tag("punctuation"));
    assert_spans_include_style(&specifier, tag("variable"));
    assert_spans_include_style(&specifier, tag("number"));
    assert_spans_include_exact_style(
        &specifier,
        specifier_line.as_str(),
        "value",
        tag("variable"),
    );

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
fn test_rust_format_string_keeps_capitalized_text_as_string() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-format-string-body", "rs", fixture);

    let spans = buf
        .syntax_spans_for_line(24)
        .expect("format string line should exist");
    let line = buf
        .line_at(24)
        .expect("format string line should exist")
        .to_text();
    let hello_start = line.find("Hello").expect("capitalized text should exist");
    let hello_end = hello_start + "Hello".len();

    assert!(spans.iter().any(|span| {
        span.start_byte <= hello_start && hello_start < span.end_byte && span.style == tag("string")
    }));
    assert!(!spans.iter().any(|span| {
        span.start_byte <= hello_start
            && hello_start < span.end_byte
            && span.style == tag("variable")
    }));
    assert!(spans.iter().any(|span| {
        span.start_byte <= hello_start && span.end_byte >= hello_end && span.style == tag("string")
    }));
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
fn test_rust_nested_block_comments_stay_in_comment_state() {
    let source = "/* outer\n   /* inner */\n   still outer\n*/\nfn nested() {}\n";
    let mut buf = fixture_buffer("syntax-rust-nested-block-comment", "rs", source);

    let outer_line = buf
        .line_at(0)
        .expect("outer comment line should exist")
        .to_string();
    let inner_line = buf
        .line_at(1)
        .expect("nested comment line should exist")
        .to_string();
    let close_line = buf
        .line_at(3)
        .expect("comment close line should exist")
        .to_string();
    let code_line = buf
        .line_at(4)
        .expect("code after block comment should exist")
        .to_string();
    let outer = buf
        .syntax_spans_for_line(0)
        .expect("outer comment line should exist");
    let inner = buf
        .syntax_spans_for_line(1)
        .expect("nested comment line should exist");
    let body = buf
        .syntax_spans_for_line(2)
        .expect("comment body line should exist");
    let close = buf
        .syntax_spans_for_line(3)
        .expect("comment close line should exist");
    let code = buf
        .syntax_spans_for_line(4)
        .expect("code after block comment should exist");

    assert_spans_include_exact_style(&outer, &outer_line, "/*", tag("comment.block"));
    assert_spans_include_exact_style(&inner, &inner_line, "/*", tag("comment.block"));
    assert_spans_include_style(&body, tag("comment.block"));
    assert_spans_include_exact_style(&close, &close_line, "*/", tag("comment.block"));
    assert_spans_include_exact_style(&code, &code_line, "fn", tag("keyword"));
}

#[test]
fn test_rust_raw_strings_keep_inner_quotes_plain() {
    let source = r###"fn main() { let raw = r##"raw "quotes" and # signs"##; }"###;
    let mut buf = fixture_buffer("syntax-rust-raw-string-nesting", "rs", source);

    let line = buf.line_at(0).expect("line should exist").to_string();
    let spans = buf.syntax_spans_for_line(0).expect("line should exist");

    assert_spans_include_exact_style(&spans, &line, "r##\"", tag("string"));
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_exact_style(&spans, &line, "\"##", tag("string"));
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
fn test_rust_function_call_highlights_function_name() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-rust-function", "rs").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("let value = compute(answer);", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("variable"));
}

#[test]
fn test_rust_fixture_highlights_global_identifiers() {
    let mut buf = fixture_buffer("syntax-rust-global", "rs", rust_fixture_source());

    let global_line_idx = line_containing(&buf, "GLOBAL_VARIABLES");
    let global_mut_line_idx = line_containing(&buf, "GLOBAL_STATE");
    let global_line = buf
        .syntax_spans_for_line(global_line_idx)
        .expect("global variable line should exist");
    let global_line_text = buf
        .line_at(global_line_idx)
        .expect("global variable line should exist")
        .to_string();
    let global_mut_line = buf
        .syntax_spans_for_line(global_mut_line_idx)
        .expect("mutable global variable line should exist");
    let global_mut_line_text = buf
        .line_at(global_mut_line_idx)
        .expect("mutable global variable line should exist")
        .to_string();

    assert_spans_include_style(&global_line, tag("keyword"));
    assert_spans_include_exact_style(
        &global_line,
        global_line_text.as_str(),
        "GLOBAL_VARIABLES",
        tag("variable.global"),
    );
    assert_spans_include_exact_style(
        &global_mut_line,
        global_mut_line_text.as_str(),
        "GLOBAL_STATE",
        tag("variable.global"),
    );
}
