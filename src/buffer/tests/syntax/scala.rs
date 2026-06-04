use super::*;

#[test]
fn test_scala_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/scala.scala");
    let mut buf = fixture_buffer("syntax-scala-fixture", "scala", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let annotation = buf
        .syntax_spans_for_line(6)
        .expect("annotation line should exist");
    let function_line = buf
        .syntax_spans_for_line(7)
        .expect("function line should exist");
    let string_line = buf
        .syntax_spans_for_line(8)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(9)
        .expect("number line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&annotation, tag("keyword"));
    assert_spans_include_style(&function_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
}

#[test]
fn test_scala_function_call_highlights_function_name() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-scala-function", "scala").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("println(value)", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function"));
}

#[test]
fn test_scala_fixture_highlights_base_and_exponent_number_literals() {
    let fixture = include_str!("fixtures/scala.scala");
    let mut buf = fixture_buffer("syntax-scala-number-literals", "scala", fixture);

    assert_scala_exact_style(&mut buf, 13, "0xFF", tag("number"));
    assert_scala_exact_style(&mut buf, 14, "0b1010_0011", tag("number"));
    assert_scala_exact_style(&mut buf, 15, "1.5e-2", tag("number"));
}

fn assert_scala_exact_style(buf: &mut Buffer, line_index: usize, fragment: &str, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();

    assert_spans_include_exact_style(&spans, line.as_str(), fragment, style);
}
