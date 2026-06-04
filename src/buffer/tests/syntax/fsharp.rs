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
