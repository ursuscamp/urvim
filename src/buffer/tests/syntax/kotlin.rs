use super::*;

#[test]
fn test_kotlin_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/kotlin.kt");
    let mut buf = fixture_buffer("syntax-kotlin-fixture", "kt", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let block_comment = buf
        .syntax_spans_for_line(2)
        .expect("block comment line should exist");
    let block_comment_text = buf
        .line_at(2)
        .expect("block comment line should exist")
        .to_string();
    let doc_comment = buf
        .syntax_spans_for_line(3)
        .expect("doc comment line should exist");
    let doc_comment_text = buf
        .line_at(3)
        .expect("doc comment line should exist")
        .to_string();
    let annotation = buf
        .syntax_spans_for_line(5)
        .expect("annotation line should exist");
    let class_line = buf
        .syntax_spans_for_line(6)
        .expect("class line should exist");
    let string_line = buf
        .syntax_spans_for_line(20)
        .expect("string line should exist");
    let constant_line = buf
        .syntax_spans_for_line(23)
        .expect("constant line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_exact_style(
        &block_comment,
        block_comment_text.as_str(),
        " Block comment ",
        tag("comment.block"),
    );
    assert_spans_include_exact_style(
        &doc_comment,
        doc_comment_text.as_str(),
        " Doc comment ",
        tag("comment.block"),
    );
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
fn test_kotlin_multiline_block_comment_styles_interior() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-kotlin-block-comment", "kt").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("/* start\nbody text\nend */\nval value = 1", path);

    let opener = buf.syntax_spans_for_line(0).expect("opener should exist");
    let body = buf.syntax_spans_for_line(1).expect("body should exist");
    let closer = buf.syntax_spans_for_line(2).expect("closer should exist");
    let code = buf.syntax_spans_for_line(3).expect("code should exist");

    assert_spans_include_exact_style(&opener, "/* start", " start", tag("comment.block"));
    assert_spans_include_exact_style(&body, "body text", "body text", tag("comment.block"));
    assert_spans_include_exact_style(&closer, "end */", "end ", tag("comment.block"));
    assert_spans_include_style(&code, tag("keyword"));
    assert_spans_include_style(&code, tag("number"));
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
