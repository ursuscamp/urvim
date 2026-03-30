use super::*;

#[test]
fn test_java_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/java.java");
    let mut buf = fixture_buffer("syntax-java-fixture", "java", fixture);

    let doc_comment = buf
        .syntax_spans_for_line(1)
        .expect("doc comment line should exist");
    let doc_comment_line = buf
        .line_at(1)
        .expect("doc comment line should exist")
        .to_string();
    let class_line = buf
        .syntax_spans_for_line(6)
        .expect("class line should exist");
    let annotation_line = buf
        .syntax_spans_for_line(7)
        .expect("annotation line should exist");
    let text_block = buf
        .syntax_spans_for_line(9)
        .expect("text block line should exist");
    let text_block_body = buf
        .syntax_spans_for_line(10)
        .expect("text block body line should exist");
    let char_line = buf
        .syntax_spans_for_line(15)
        .expect("char line should exist");
    let number_line = buf
        .syntax_spans_for_line(16)
        .expect("number line should exist");
    let constant_line = buf
        .syntax_spans_for_line(17)
        .expect("constant line should exist");

    assert_spans_include_style(&doc_comment, tag("comment.documentation"));
    let body_start = doc_comment_line
        .find("doc comment")
        .expect("doc comment body should exist");
    let body_end = body_start + "doc comment".len();
    assert!(doc_comment.iter().any(|span| {
        span.start_byte <= body_start
            && span.end_byte >= body_end
            && span.style == tag("comment.documentation")
    }));
    assert_spans_include_style(&class_line, tag("keyword"));
    assert_spans_include_style(&class_line, tag("type"));
    assert_spans_include_style(&annotation_line, tag("keyword"));
    assert_spans_include_style(&text_block, tag("string"));
    assert_spans_include_style(&text_block_body, tag("string"));
    assert_spans_include_style(&char_line, tag("constant"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&constant_line, tag("constant"));
}
