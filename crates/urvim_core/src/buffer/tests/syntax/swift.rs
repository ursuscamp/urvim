use super::*;

#[test]
fn test_swift_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/swift.swift");
    let mut buf = fixture_buffer("syntax-swift-fixture", "swift", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let annotation = buf
        .syntax_spans_for_line(6)
        .expect("annotation line should exist");
    let struct_line = buf
        .syntax_spans_for_line(7)
        .expect("struct line should exist");
    let string_line = buf
        .syntax_spans_for_line(8)
        .expect("string line should exist");
    let constant_line = buf
        .syntax_spans_for_line(11)
        .expect("constant line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&annotation, tag("keyword"));
    assert_spans_include_style(&struct_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&constant_line, tag("constant"));
}

#[test]
fn test_swift_function_call_highlights_function_name() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-swift-function", "swift").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("render(message)", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function"));
}

#[test]
fn test_swift_fixture_highlights_block_comment_contents() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/swift.swift");
    let mut buf = fixture_buffer("syntax-swift-block-comment", "swift", fixture);

    assert_swift_exact_style(&mut buf, 2, "/*", tag("comment.block"));
    assert_swift_exact_style(&mut buf, 2, " Multi-line", tag("comment.block"));
    assert_swift_exact_style(&mut buf, 3, "   block comment ", tag("comment.block"));
    assert_swift_exact_style(&mut buf, 3, "*/", tag("comment.block"));
}

#[test]
fn test_swift_fixture_highlights_base_and_exponent_number_literals() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/swift.swift");
    let mut buf = fixture_buffer("syntax-swift-number-literals", "swift", fixture);

    assert_swift_exact_style(&mut buf, 14, "0xFF", tag("number"));
    assert_swift_exact_style(&mut buf, 15, "0b1010_0011", tag("number"));
    assert_swift_exact_style(&mut buf, 16, "0o77", tag("number"));
    assert_swift_exact_style(&mut buf, 17, "1.5e-2", tag("number"));
}

fn assert_swift_exact_style(buf: &mut Buffer, line_index: usize, fragment: &str, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();

    assert_spans_include_exact_style(&spans, line.as_str(), fragment, style);
}
