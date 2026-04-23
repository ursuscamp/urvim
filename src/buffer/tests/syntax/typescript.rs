use super::*;

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
    assert_spans_include_style(&jsx_line, tag("markup.tag"));
    assert_spans_include_style(&jsx_line, tag("variable.property"));
    assert_spans_include_style(&jsx_line, tag("string"));
}

#[test]
fn test_typescript_tsx_lines_look_like_jsx() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-typescript-tsx", "tsx").as_path())
            .unwrap();
    let mut buf =
        Buffer::from_str_with_path("const view = <Button kind=\"primary\" disabled />;", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("markup.tag"));
    assert_spans_include_style(&spans, tag("variable.property"));
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
}
