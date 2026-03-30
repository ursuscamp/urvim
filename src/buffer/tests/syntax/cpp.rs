use super::*;

#[test]
fn test_cpp_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/cpp.cpp");
    let mut buf = fixture_buffer("syntax-cpp-fixture", "cpp", fixture);

    let namespace_line = buf
        .syntax_spans_for_line(2)
        .expect("namespace line should exist");
    let template_line = buf
        .syntax_spans_for_line(3)
        .expect("template line should exist");
    let raw_string_line = buf
        .syntax_spans_for_line(14)
        .expect("raw string line should exist");
    let raw_string_body = buf
        .syntax_spans_for_line(15)
        .expect("raw string body line should exist");
    let constants_line = buf
        .syntax_spans_for_line(16)
        .expect("constants line should exist");
    let printf_line = buf
        .syntax_spans_for_line(18)
        .expect("printf line should exist");
    let fprintf_line = buf
        .syntax_spans_for_line(19)
        .expect("fprintf line should exist");

    assert_spans_include_style(&namespace_line, tag("keyword"));
    assert_spans_include_style(&namespace_line, tag("punctuation"));
    assert_spans_include_style(&template_line, tag("keyword"));
    assert_spans_include_style(&template_line, tag("type"));
    assert_spans_include_style(&raw_string_line, tag("string"));
    assert_spans_include_style(&raw_string_body, tag("string"));
    assert_spans_include_style(&constants_line, tag("constant"));
    assert_spans_include_style(&constants_line, tag("keyword"));
    assert_spans_include_style(&printf_line, tag("function"));
    assert_spans_include_style(&printf_line, tag("punctuation"));
    assert_spans_include_style(&printf_line, tag("string"));
    assert_spans_include_style(&printf_line, tag("string.interpolation"));
    assert_spans_include_style(&printf_line, tag("string.escape"));
    assert_spans_include_style(&fprintf_line, tag("function"));
    assert_spans_include_style(&fprintf_line, tag("punctuation"));
    assert_spans_include_style(&fprintf_line, tag("string"));
    assert_spans_include_style(&fprintf_line, tag("string.interpolation"));
}

#[test]
fn test_cpp_printf_format_string_only_applies_to_first_string_argument() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-cpp-printf", "cpp").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path(
        "std::fprintf(stderr, \"%s %s\", \"first\", \"second\");",
        path,
    );

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    let line = buf.line_at(0).expect("line should exist");

    assert_spans_include_style(&spans, tag("function"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("string.interpolation"));

    let first_open = line.find('"').expect("format string should open");
    let first_close = line[first_open + 1..]
        .find('"')
        .map(|index| index + first_open + 1)
        .expect("format string should close");
    let first_string = spans
        .iter()
        .filter(|span| span.start_byte >= first_open && span.end_byte <= first_close + 1)
        .collect::<Vec<_>>();
    assert!(
        first_string
            .iter()
            .any(|span| span.style == tag("string.interpolation"))
    );

    let second_close = line.rfind('"').expect("second string should close");
    let second_open = line[..second_close]
        .rfind('"')
        .expect("second string should open");
    let second_string = spans
        .iter()
        .filter(|span| span.start_byte >= second_open && span.end_byte <= second_close + 1)
        .collect::<Vec<_>>();
    assert!(
        !second_string
            .iter()
            .any(|span| span.style == tag("string.interpolation"))
    );
    assert!(second_string.iter().any(|span| span.style == tag("string")));
}

#[test]
fn test_cpp_printf_format_string_keeps_plain_text_as_string() {
    let fixture = include_str!("fixtures/cpp.cpp");
    let mut buf = fixture_buffer("syntax-cpp-printf-string-body", "cpp", fixture);

    let spans = buf
        .syntax_spans_for_line(18)
        .expect("printf line should exist");
    let line = buf.line_at(18).expect("printf line should exist");
    let value_start = line.find("value=").expect("printf body text should exist");
    let value_end = value_start + "value=".len();

    assert!(spans.iter().any(|span| {
        span.start_byte <= value_start && value_start < span.end_byte && span.style == tag("string")
    }));
    assert!(!spans.iter().any(|span| {
        span.start_byte <= value_start
            && value_start < span.end_byte
            && span.style == tag("string.interpolation")
    }));
    assert!(spans.iter().any(|span| {
        span.start_byte <= value_start && span.end_byte >= value_end && span.style == tag("string")
    }));
}
