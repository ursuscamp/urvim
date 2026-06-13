use super::*;

fn assert_javascript_folds(source: &str, expected: &[(usize, usize)]) {
    let mut buf = fixture_buffer("syntax-js-folds", "js", source);
    let actual: Vec<(usize, usize)> = buf
        .syntax_fold_regions()
        .iter()
        .map(|region| (region.start_line, region.end_line))
        .collect();
    assert_eq!(actual, expected);
}

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
fn test_javascript_bracket_folds_ignore_strings_and_comments() {
    assert_javascript_folds(
        "function demo() {\n  const text = \"{\";\n  // {\n  if (text) {\n    return [\n      text,\n    ];\n  }\n}\n",
        &[(4, 6), (3, 7), (0, 8)],
    );
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
    let call_line = buf
        .syntax_spans_for_line(4)
        .expect("call line should exist");
    let object_line = buf
        .syntax_spans_for_line(5)
        .expect("object line should exist");
    let template_line = buf
        .syntax_spans_for_line(6)
        .expect("template line should exist");
    let operator_line = buf
        .syntax_spans_for_line(12)
        .expect("operator line should exist");
    let jsx_line = buf
        .syntax_spans_for_line(32)
        .expect("jsx line should exist");
    let jsx_text = buf.line_at(32).expect("jsx line should exist").to_string();

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&keyword_line, tag("keyword"));
    assert_spans_include_style(&keyword_line, tag("punctuation"));
    assert_spans_include_style(&call_line, tag("function"));
    assert_spans_include_style(&object_line, tag("punctuation"));
    assert_spans_include_style(&object_line, tag("constant"));
    assert_spans_include_style(&operator_line, tag("operator"));
    assert_spans_include_style(&operator_line, tag("constant"));
    assert_exact_spans(
        &template_line,
        &[
            (2, 7, tag("keyword")),
            (8, 15, tag("variable")),
            (16, 17, tag("operator")),
            (18, 19, tag("string")),
            (19, 25, tag("string")),
            (25, 27, tag("punctuation")),
            (27, 30, tag("variable")),
            (30, 31, tag("punctuation")),
            (31, 32, tag("string")),
            (32, 33, tag("punctuation")),
        ],
    );
    assert_spans_include_exact_style(&jsx_line, jsx_text.as_str(), "<", tag("punctuation"));
    assert_spans_include_exact_style(&jsx_line, jsx_text.as_str(), "div", tag("markup.tag"));
    assert_spans_include_exact_style(
        &jsx_line,
        jsx_text.as_str(),
        "className",
        tag("variable.property"),
    );
    assert_spans_include_exact_style(
        &jsx_line,
        jsx_text.as_str(),
        "disabled",
        tag("variable.property"),
    );
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

#[test]
fn test_javascript_jsx_highlights_tags_and_attributes() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-js-jsx", "js").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path(
        "const view = <><div className=\"app\" hidden>{value}</div><Button disabled /></>;",
        path,
    );
    let line = buf.line_at(0).expect("line should exist").to_string();

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_exact_style(&spans, line.as_str(), "<", tag("punctuation"));
    assert_spans_include_exact_style(&spans, line.as_str(), "div", tag("markup.tag"));
    assert_spans_include_exact_style(&spans, line.as_str(), "/", tag("punctuation"));
    assert_spans_include_exact_style(&spans, line.as_str(), "Button", tag("markup.tag"));
    assert_spans_include_exact_style(&spans, line.as_str(), "className", tag("variable.property"));
    assert_spans_include_exact_style(&spans, line.as_str(), "hidden", tag("variable.property"));
    assert_spans_include_exact_style(&spans, line.as_str(), "disabled", tag("variable.property"));
    assert_spans_include_exact_style(&spans, line.as_str(), "value", tag("variable"));
    assert_jsx_delimiter_style(&spans, line.as_str(), "</div>", ">", tag("punctuation"));
    assert_jsx_delimiter_style(
        &spans,
        line.as_str(),
        "disabled />",
        "/",
        tag("punctuation"),
    );
    assert_jsx_delimiter_style(
        &spans,
        line.as_str(),
        "disabled />",
        ">",
        tag("punctuation"),
    );
    assert_spans_include_style(&spans, tag("string"));
}

fn assert_jsx_delimiter_style(
    spans: &[crate::buffer::syntax::SyntaxSpan],
    line: &str,
    containing_fragment: &str,
    delimiter: &str,
    style: Tag,
) {
    let fragment_start = line
        .find(containing_fragment)
        .unwrap_or_else(|| panic!("expected line to contain fragment {containing_fragment:?}"));
    let delimiter_start = line[fragment_start..]
        .find(delimiter)
        .map(|offset| fragment_start + offset)
        .unwrap_or_else(|| panic!("expected fragment to contain delimiter {delimiter:?}"));
    let delimiter_end = delimiter_start + delimiter.len();

    assert!(
        spans.iter().any(|span| {
            span.start_byte == delimiter_start
                && span.end_byte == delimiter_end
                && span.style == style
        }),
        "expected delimiter {delimiter:?} in {containing_fragment:?} at {delimiter_start}..{delimiter_end} to use style {style:?}"
    );
}
