use super::*;

#[test]
fn test_justfile_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/justfile");
    let mut buf = named_fixture_buffer("justfile", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let recipe = buf
        .syntax_spans_for_line(3)
        .expect("recipe line should exist");
    let call = buf
        .syntax_spans_for_line(11)
        .expect("call line should exist");
    let assignment = buf
        .syntax_spans_for_line(5)
        .expect("assignment line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&recipe, tag("keyword"));
    assert_spans_include_style(&call, tag("variable"));
    assert_spans_include_style(&call, tag("string"));
    assert_spans_include_style(&assignment, tag("operator"));
}

#[test]
fn test_justfile_fixture_highlights_recipe_target_boundary() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/justfile");
    let mut buf = named_fixture_buffer("justfile", fixture);

    let line = buf
        .syntax_spans_for_line(33)
        .expect("dependency recipe line should exist");
    let text = fixture
        .lines()
        .nth(33)
        .expect("dependency recipe line should exist")
        .to_string();

    assert_spans_include_exact_style(&line, text.as_str(), "release:", tag("keyword"));
}
