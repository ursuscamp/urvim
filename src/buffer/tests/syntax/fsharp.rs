use super::*;

#[test]
fn test_fsharp_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/fsharp.fs");
    let mut buf = fixture_buffer("syntax-fsharp-fixture", "fs", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let module_line = buf
        .syntax_spans_for_line(6)
        .expect("module line should exist");
    let string_line = buf
        .syntax_spans_for_line(21)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(24)
        .expect("number line should exist");
    let char_line = buf
        .syntax_spans_for_line(25)
        .expect("char line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&module_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&char_line, tag("constant"));
}

#[test]
fn test_fsharp_fixture_highlights_block_comment_body() {
    let fixture = include_str!("fixtures/fsharp.fs");
    let mut buf = fixture_buffer("syntax-fsharp-block-comment", "fs", fixture);

    assert_fsharp_exact_style(&mut buf, 2, "(*", tag("comment.block"));
    assert_fsharp_exact_style(&mut buf, 2, " Multi-line", tag("comment.block"));
    assert_fsharp_exact_style(&mut buf, 3, "   block comment ", tag("comment.block"));
    assert_fsharp_exact_style(&mut buf, 3, "*)", tag("comment.block"));
}

#[test]
fn test_fsharp_fixture_highlights_octal_numbers() {
    let fixture = include_str!("fixtures/fsharp.fs");
    let mut buf = fixture_buffer("syntax-fsharp-octal", "fs", fixture);

    assert_fsharp_exact_style(&mut buf, 29, "0o77", tag("number"));
}

fn assert_fsharp_exact_style(buf: &mut Buffer, line_index: usize, fragment: &str, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();

    assert_spans_include_exact_style(&spans, line.as_str(), fragment, style);
}
