use super::*;

#[test]
fn test_python_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/python.py");
    let mut buf = fixture_buffer("syntax-py-fixture", "py", fixture);

    let docstring = buf
        .syntax_spans_for_line(1)
        .expect("docstring line should exist");
    let comment = buf
        .syntax_spans_for_line(6)
        .expect("comment line should exist");
    let definition = buf
        .syntax_spans_for_line(8)
        .expect("definition line should exist");
    let mapping = buf
        .syntax_spans_for_line(21)
        .expect("mapping line should exist");

    assert_spans_include_style(&docstring, tag("string"));
    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&definition, tag("keyword"));
    assert_spans_include_style(&definition, tag("type"));
    assert_spans_include_style(&definition, tag("punctuation"));
    assert_spans_include_style(&definition, tag("operator"));
    assert_spans_include_style(&mapping, tag("punctuation"));
    assert_spans_include_style(&mapping, tag("constant"));
}

#[test]
fn test_python_fixture_highlights_extended_prefixes() {
    let fixture = include_str!("fixtures/python.py");
    let mut buf = fixture_buffer("syntax-python-extended", "py", fixture);

    let decorator = buf
        .syntax_spans_for_line(28)
        .expect("decorator line should exist");
    let raw_string = buf
        .syntax_spans_for_line(30)
        .expect("raw string line should exist");
    let bytes_string = buf
        .syntax_spans_for_line(31)
        .expect("bytes string line should exist");
    let raw_bytes = buf
        .syntax_spans_for_line(33)
        .expect("raw bytes line should exist");
    let combined = buf
        .syntax_spans_for_line(34)
        .expect("combined f-string line should exist");
    let raw_combined = buf
        .syntax_spans_for_line(35)
        .expect("raw combined f-string line should exist");
    let _numeric = buf
        .syntax_spans_for_line(36)
        .expect("numeric line should exist");
    let raw_multiline_start = buf
        .syntax_spans_for_line(41)
        .expect("raw multiline start line should exist");
    let raw_multiline_body = buf
        .syntax_spans_for_line(42)
        .expect("raw multiline body line should exist");

    assert_spans_include_style(&decorator, tag("keyword"));
    assert_spans_include_style(&raw_string, tag("string"));
    assert_spans_include_style(&bytes_string, tag("string"));
    assert_spans_include_style(&raw_bytes, tag("string"));
    assert_spans_include_style(&combined, tag("string"));
    assert_spans_include_style(&raw_combined, tag("string"));
    assert_spans_include_style(&raw_multiline_start, tag("string"));
    assert_spans_include_style(&raw_multiline_body, tag("string"));
}

#[test]
fn test_python_fstring_highlights_interpolation_body() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-py-fstring", "py").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("msg = f\"hello {1 + 2}\"", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("number"));
    assert_spans_include_style(&spans, tag("operator"));
}

#[test]
fn test_python_multiline_fstring_closing_brace_is_punctuation() {
    let fixture = include_str!("fixtures/python.py");
    let mut buf = fixture_buffer("syntax-py-multiline-fstring", "py", fixture);

    let spans = buf
        .syntax_spans_for_line(25)
        .expect("multiline f-string interpolation line should exist");
    let line = buf
        .line_at(25)
        .expect("multiline f-string interpolation line should exist");
    let close_brace = line
        .rfind('}')
        .expect("interpolation close brace should exist");

    assert_spans_include_style(&spans, tag("punctuation"));
    assert!(spans.iter().any(|span| {
        span.start_byte <= close_brace
            && close_brace < span.end_byte
            && span.style == tag("punctuation")
    }));
    assert!(!spans.iter().any(|span| {
        span.start_byte <= close_brace && close_brace < span.end_byte && span.style == tag("string")
    }));
}

#[test]
fn test_python_constants_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-py-constant", "py").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("value = True\nmissing = None", path);

    let first_line = buf
        .syntax_spans_for_line(0)
        .expect("first line should exist");
    let second_line = buf
        .syntax_spans_for_line(1)
        .expect("second line should exist");
    assert_spans_include_style(&first_line, tag("constant"));
    assert_spans_include_style(&second_line, tag("constant"));
}

#[test]
fn test_python_types_use_type_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-py-type", "py").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("class Thing(Exception):\n    pass", path);

    let first_line = buf
        .syntax_spans_for_line(0)
        .expect("first line should exist");
    assert_spans_include_style(&first_line, tag("keyword"));
    assert_spans_include_style(&first_line, tag("type"));
}
