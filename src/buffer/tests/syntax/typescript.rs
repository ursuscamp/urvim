use super::*;

fn assert_typescript_folds(source: &str, expected: &[(usize, usize)]) {
    let mut buf = fixture_buffer("syntax-typescript-folds", "ts", source);
    let actual: Vec<(usize, usize)> = buf
        .syntax_fold_regions()
        .iter()
        .map(|region| (region.start_line, region.end_line))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn test_typescript_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/typescript.ts");
    let mut buf = fixture_buffer("syntax-typescript-fixture", "ts", fixture);

    let interface_line = buf
        .syntax_spans_for_line(3)
        .expect("interface line should exist");
    let decorator_line = buf
        .syntax_spans_for_line(8)
        .expect("decorator line should exist");
    let function_line = buf
        .syntax_spans_for_line(13)
        .expect("function line should exist");
    let call_line = buf
        .syntax_spans_for_line(18)
        .expect("call line should exist");
    let template_line = buf
        .syntax_spans_for_line(15)
        .expect("template line should exist");
    let template_text = buf
        .line_at(15)
        .expect("template line should exist")
        .to_string();
    let jsx_line = buf
        .syntax_spans_for_line(22)
        .expect("jsx line should exist");

    assert_spans_include_style(&interface_line, tag("keyword"));
    assert_spans_include_style(&interface_line, tag("type"));
    assert_spans_include_style(&decorator_line, tag("keyword"));
    assert_spans_include_style(&function_line, tag("keyword"));
    assert_spans_include_style(&function_line, tag("function"));
    assert_spans_include_style(&function_line, tag("type"));
    assert_spans_include_style(&call_line, tag("function"));
    assert_spans_include_style(&template_line, tag("string"));
    assert_spans_include_style(&template_line, tag("punctuation"));
    assert_spans_include_exact_style(
        &template_line,
        template_text.as_str(),
        "value",
        tag("variable"),
    );
    assert_exact_spans(
        &jsx_line,
        &[
            (0, 5, tag("keyword")),
            (6, 10, tag("variable")),
            (11, 12, tag("operator")),
            (13, 14, tag("punctuation")),
            (14, 20, tag("markup.tag")),
            (21, 25, tag("variable.property")),
            (25, 26, tag("operator")),
            (26, 27, tag("string")),
            (27, 34, tag("string")),
            (34, 35, tag("string")),
            (36, 44, tag("variable.property")),
            (45, 46, tag("punctuation")),
            (46, 47, tag("punctuation")),
            (47, 48, tag("punctuation")),
        ],
    );
    assert_spans_include_style(&jsx_line, tag("markup.tag"));
    assert_spans_include_style(&jsx_line, tag("variable.property"));
    assert_spans_include_style(&jsx_line, tag("string"));
}

#[test]
fn test_typescript_bracket_folds_ignore_strings_and_comments() {
    assert_typescript_folds(
        "type Item = {\n  value: string;\n};\nfunction demo(items: Item[]) {\n  const text = `)`;\n  // {\n  return items.map((item) => {\n    return item.value;\n  });\n}\n",
        &[(0, 2), (6, 8), (3, 9)],
    );
}

#[test]
fn test_typescript_tsx_lines_look_like_jsx() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-typescript-tsx", "tsx").as_path())
            .unwrap();
    let mut buf =
        Buffer::from_str_with_path("const view = <Button kind=\"primary\" disabled />;", path);
    let line = buf.line_at(0).expect("line should exist").to_string();

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("markup.tag"));
    assert_spans_include_style(&spans, tag("variable.property"));
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_exact_style(&spans, line.as_str(), "disabled", tag("variable.property"));
}

#[test]
fn test_typescript_jsx_highlights_lowercase_tags_fragments_and_qualified_names() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-typescript-jsx", "ts").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path(
        "const view = <><div className=\"app\" hidden>{message}</div><UI.Card /><svg:path /></>;",
        path,
    );
    let line = buf.line_at(0).expect("line should exist").to_string();

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_exact_style(&spans, line.as_str(), "<", tag("punctuation"));
    assert_spans_include_exact_style(&spans, line.as_str(), "div", tag("markup.tag"));
    assert_spans_include_exact_style(&spans, line.as_str(), "/", tag("punctuation"));
    assert_spans_include_exact_style(&spans, line.as_str(), "UI.Card", tag("markup.tag"));
    assert_spans_include_exact_style(&spans, line.as_str(), "svg:path", tag("markup.tag"));
    assert_spans_include_exact_style(&spans, line.as_str(), "className", tag("variable.property"));
    assert_spans_include_exact_style(&spans, line.as_str(), "hidden", tag("variable.property"));
    assert_spans_include_exact_style(&spans, line.as_str(), "message", tag("variable"));
    assert_jsx_delimiter_style(&spans, line.as_str(), "</div>", ">", tag("punctuation"));
    assert_jsx_delimiter_style(&spans, line.as_str(), "UI.Card />", "/", tag("punctuation"));
    assert_jsx_delimiter_style(&spans, line.as_str(), "UI.Card />", ">", tag("punctuation"));
    assert_jsx_delimiter_style(
        &spans,
        line.as_str(),
        "svg:path />",
        "/",
        tag("punctuation"),
    );
    assert_jsx_delimiter_style(
        &spans,
        line.as_str(),
        "svg:path />",
        ">",
        tag("punctuation"),
    );
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
