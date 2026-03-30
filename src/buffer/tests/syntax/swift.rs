use super::*;

#[test]
fn test_swift_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/swift.swift");
    let mut buf = fixture_buffer("syntax-swift-fixture", "swift", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let annotation = buf
        .syntax_spans_for_line(1)
        .expect("annotation line should exist");
    let struct_line = buf
        .syntax_spans_for_line(2)
        .expect("struct line should exist");
    let string_line = buf
        .syntax_spans_for_line(3)
        .expect("string line should exist");
    let constant_line = buf
        .syntax_spans_for_line(6)
        .expect("constant line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&annotation, tag("keyword"));
    assert_spans_include_style(&struct_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&constant_line, tag("constant"));
}
