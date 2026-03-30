use super::*;

#[test]
fn test_justfile_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/justfile");
    let mut buf = named_fixture_buffer("justfile", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let recipe = buf
        .syntax_spans_for_line(1)
        .expect("recipe line should exist");
    let call = buf
        .syntax_spans_for_line(2)
        .expect("call line should exist");
    let assignment = buf
        .syntax_spans_for_line(3)
        .expect("assignment line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&recipe, tag("keyword"));
    assert_spans_include_style(&call, tag("variable"));
    assert_spans_include_style(&call, tag("string"));
    assert_spans_include_style(&assignment, tag("operator"));
}
