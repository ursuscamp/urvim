use super::*;

#[test]
fn test_makefile_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/Makefile");
    let mut buf = named_fixture_buffer("makefile", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let target = buf
        .syntax_spans_for_line(3)
        .expect("target line should exist");
    let variable = buf
        .syntax_spans_for_line(5)
        .expect("variable line should exist");
    let include_line = buf
        .syntax_spans_for_line(11)
        .expect("include line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&target, tag("keyword"));
    assert_spans_include_style(&variable, tag("operator"));
    assert_spans_include_style(&include_line, tag("keyword"));
}
