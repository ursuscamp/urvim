use super::*;

#[test]
fn test_javascript_types_use_type_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-type", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("class Thing extends Error {}", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("keyword"));
    assert_spans_include_style(&spans, tag("type"));
}

#[test]
fn test_javascript_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/javascript.js");
    let mut buf = fixture_buffer("syntax-js-fixture", "js", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let keyword_line = buf
        .syntax_spans_for_line(3)
        .expect("keyword line should exist");
    let object_line = buf
        .syntax_spans_for_line(5)
        .expect("object line should exist");
    let operator_line = buf
        .syntax_spans_for_line(12)
        .expect("operator line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&keyword_line, tag("keyword"));
    assert_spans_include_style(&keyword_line, tag("punctuation"));
    assert_spans_include_style(&object_line, tag("punctuation"));
    assert_spans_include_style(&object_line, tag("constant"));
    assert_spans_include_style(&operator_line, tag("operator"));
    assert_spans_include_style(&operator_line, tag("constant"));
}

#[test]
fn test_javascript_fixture_highlights_regex_private_fields_and_bigints() {
    let fixture = include_str!("fixtures/javascript.js");
    let mut buf = fixture_buffer("syntax-js-extended", "js", fixture);

    let regex_line = buf
        .syntax_spans_for_line(19)
        .expect("regex line should exist");
    let bigint_line = buf
        .syntax_spans_for_line(20)
        .expect("bigint line should exist");
    let private_field = buf
        .syntax_spans_for_line(26)
        .expect("private field line should exist");
    let private_access = buf
        .syntax_spans_for_line(28)
        .expect("private access line should exist");

    assert_spans_include_style(&regex_line, tag("string"));
    assert_spans_include_style(&bigint_line, tag("number"));
    assert_spans_include_style(&private_field, tag("variable"));
    assert_spans_include_style(&private_access, tag("variable"));
}

#[test]
fn test_javascript_template_string_highlights_interpolation_body() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-template", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("const msg = `hi ${1 + 2} there`;", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("number"));
    assert_spans_include_style(&spans, tag("operator"));
}

#[test]
fn test_javascript_escape_sequences_use_escape_regions() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-escape", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("const msg = \"line 1\\nline 2\";", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
}

#[test]
fn test_javascript_constants_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-constant", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("const value = null;", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("keyword"));
    assert_spans_include_style(&spans, tag("constant"));
}
