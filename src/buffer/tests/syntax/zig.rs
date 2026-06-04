use super::*;

#[test]
fn test_zig_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/zig.zig");
    let mut buf = fixture_buffer("syntax-zig-fixture", "zig", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let import_line = buf
        .syntax_spans_for_line(4)
        .expect("import line should exist");
    let const_line = buf
        .syntax_spans_for_line(9)
        .expect("const line should exist");
    let string_line = buf
        .syntax_spans_for_line(11)
        .expect("string line should exist");
    let call_line = buf
        .syntax_spans_for_line(44)
        .expect("call line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&import_line, tag("keyword"));
    assert_spans_include_style(&import_line, tag("function.macro"));
    assert_spans_include_style(&import_line, tag("string"));
    assert_spans_include_style(&const_line, tag("keyword"));
    assert_spans_include_style(&const_line, tag("type"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&call_line, tag("function"));
}
