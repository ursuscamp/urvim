use super::*;

#[test]
fn test_r_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/r.r");
    let mut buf = fixture_buffer("syntax-r-fixture", "r", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let string_line = buf
        .syntax_spans_for_line(4)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(5)
        .expect("number line should exist");
    let constant_line = buf
        .syntax_spans_for_line(6)
        .expect("constant line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&constant_line, tag("constant"));
}
