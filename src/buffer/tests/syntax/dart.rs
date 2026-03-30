use super::*;

#[test]
fn test_dart_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/dart.dart");
    let mut buf = fixture_buffer("syntax-dart-fixture", "dart", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let annotation = buf
        .syntax_spans_for_line(1)
        .expect("annotation line should exist");
    let class_line = buf
        .syntax_spans_for_line(2)
        .expect("class line should exist");
    let raw_line = buf
        .syntax_spans_for_line(4)
        .expect("raw string line should exist");
    let number_line = buf
        .syntax_spans_for_line(7)
        .expect("number line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&annotation, tag("keyword"));
    assert_spans_include_style(&class_line, tag("keyword"));
    assert_spans_include_style(&class_line, tag("type"));
    assert_spans_include_style(&raw_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
}
