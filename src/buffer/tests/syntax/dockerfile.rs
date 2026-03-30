use super::*;

#[test]
fn test_dockerfile_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/Dockerfile");
    let mut buf = named_fixture_buffer("dockerfile", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let from_line = buf
        .syntax_spans_for_line(1)
        .expect("from line should exist");
    let env_line = buf
        .syntax_spans_for_line(3)
        .expect("variable line should exist");
    let run_line = buf.syntax_spans_for_line(4).expect("run line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&from_line, tag("keyword"));
    assert_spans_include_style(&env_line, tag("variable"));
    assert_spans_include_style(&env_line, tag("keyword"));
    assert_spans_include_style(&run_line, tag("string"));
}
