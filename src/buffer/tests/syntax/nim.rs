use super::*;

#[test]
fn test_nim_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/nim.nim");
    let mut buf = fixture_buffer("syntax-nim-fixture", "nim", fixture);

    let doc_comment = buf
        .syntax_spans_for_line(0)
        .expect("doc comment line should exist");
    let block_comment = buf
        .syntax_spans_for_line(4)
        .expect("block comment line should exist");
    let type_line = buf
        .syntax_spans_for_line(8)
        .expect("type line should exist");
    let number_line = buf
        .syntax_spans_for_line(19)
        .expect("number line should exist");
    let string_line = buf
        .syntax_spans_for_line(28)
        .expect("string line should exist");

    assert_spans_include_comment_style(&doc_comment);
    assert_spans_include_comment_style(&block_comment);
    assert_spans_include_style(&type_line, tag("keyword"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&string_line, tag("string"));
}

#[test]
fn test_nim_fixture_highlights_block_comment_body() {
    let fixture = include_str!("fixtures/nim.nim");
    let mut buf = fixture_buffer("syntax-nim-block-comment", "nim", fixture);

    assert_nim_line_has_style(&mut buf, 4, tag("comment.block"));
    assert_nim_exact_style(&mut buf, 4, " block comment ", tag("comment.block"));
    assert_nim_line_has_style(&mut buf, 5, tag("comment.block"));
    assert_nim_exact_style(&mut buf, 5, " multi-line", tag("comment.block"));
    assert_nim_line_has_style(&mut buf, 6, tag("comment.block"));
    assert_nim_exact_style(&mut buf, 6, "   block comment ", tag("comment.block"));
    assert_nim_exact_style(&mut buf, 6, "]#", tag("comment.block"));
}

#[test]
fn test_nim_fixture_highlights_base_prefixed_numbers() {
    let fixture = include_str!("fixtures/nim.nim");
    let mut buf = fixture_buffer("syntax-nim-number-literals", "nim", fixture);

    assert_nim_exact_style(&mut buf, 24, "0xFF", tag("number"));
    assert_nim_exact_style(&mut buf, 25, "0b1010_0011", tag("number"));
    assert_nim_exact_style(&mut buf, 26, "0o77", tag("number"));
}

fn assert_nim_exact_style(buf: &mut Buffer, line_index: usize, fragment: &str, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();

    assert_spans_include_exact_style(&spans, line.as_str(), fragment, style);
}

fn assert_nim_line_has_style(buf: &mut Buffer, line_index: usize, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));

    assert_spans_include_style(&spans, style);
}
