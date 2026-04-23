use super::*;

#[test]
fn test_kotlin_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/kotlin.kt");
    let mut buf = fixture_buffer("syntax-kotlin-fixture", "kt", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let annotation = buf
        .syntax_spans_for_line(1)
        .expect("annotation line should exist");
    let class_line = buf
        .syntax_spans_for_line(2)
        .expect("class line should exist");
    let string_line = buf
        .syntax_spans_for_line(3)
        .expect("string line should exist");
    let constant_line = buf
        .syntax_spans_for_line(6)
        .expect("constant line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&annotation, tag("keyword"));
    assert_spans_include_style(&class_line, tag("keyword"));
    assert_spans_include_style(&class_line, tag("type"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&constant_line, tag("constant"));

    let number_path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-kotlin-number", "kt").as_path())
            .unwrap();
    let mut number_buf = Buffer::from_str_with_path("val answer = 42", number_path);
    let number_line = number_buf
        .syntax_spans_for_line(0)
        .expect("number line should exist");

    assert_spans_include_style(&number_line, tag("number"));
}

#[test]
fn test_kotlin_function_call_highlights_function_name() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-kotlin-function", "kt").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("render(message)", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function"));
}
