use super::*;

#[test]
fn test_scala_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/scala.scala");
    let mut buf = fixture_buffer("syntax-scala-fixture", "scala", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let annotation = buf
        .syntax_spans_for_line(1)
        .expect("annotation line should exist");
    let function_line = buf
        .syntax_spans_for_line(2)
        .expect("function line should exist");
    let string_line = buf
        .syntax_spans_for_line(3)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(4)
        .expect("number line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&annotation, tag("keyword"));
    assert_spans_include_style(&function_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
}
