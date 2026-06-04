use super::*;

#[test]
fn test_c_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/c.c");
    let mut buf = fixture_buffer("syntax-c-fixture", "c", fixture);

    let comment = buf
        .syntax_spans_for_line(1)
        .expect("comment line should exist");
    let preprocessor = buf
        .syntax_spans_for_line(2)
        .expect("preprocessor line should exist");
    let definition = buf
        .syntax_spans_for_line(5)
        .expect("definition line should exist");
    let literal = buf
        .syntax_spans_for_line(6)
        .expect("literal line should exist");
    let number_line = buf
        .syntax_spans_for_line(7)
        .expect("number line should exist");
    let call = buf
        .syntax_spans_for_line(8)
        .expect("call line should exist");
    let _formatted_call = buf
        .syntax_spans_for_line(9)
        .expect("formatted call line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&preprocessor, tag("keyword"));
    assert_spans_include_style(&definition, tag("type"));
    assert_spans_include_style(&literal, tag("constant"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&call, tag("function"));
}

#[test]
fn test_c_printf_format_string_only_applies_to_first_string_argument() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-c-printf", "c").as_path()).unwrap();
    let mut buf =
        Buffer::from_str_with_path("fprintf(stderr, \"error=%d: %s\\n\", 7, \"tail\");", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    let line = buf.line_at(0).expect("line should exist").to_text();

    assert_spans_include_style(&spans, tag("function"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("string.interpolation"));
    assert_spans_include_style(&spans, tag("string.escape"));

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
fn test_c_printf_format_string_keeps_plain_text_as_string() {
    let fixture = include_str!("fixtures/c.c");
    let mut buf = fixture_buffer("syntax-c-printf-string-body", "c", fixture);

    let spans = buf
        .syntax_spans_for_line(8)
        .expect("printf line should exist");
    let line = buf.line_at(8).expect("printf line should exist").to_text();
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

#[test]
fn test_c_function_call_highlights_function_name() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-c-function", "c").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("compute(value);", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function"));
}
