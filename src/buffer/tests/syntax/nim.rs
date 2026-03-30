use super::*;

#[test]
fn test_nim_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/nim.nim");
    let mut buf = fixture_buffer("syntax-nim-fixture", "nim", fixture);

    let doc_comment = buf
        .syntax_spans_for_line(0)
        .expect("doc comment line should exist");
    let block_comment = buf
        .syntax_spans_for_line(1)
        .expect("block comment line should exist");
    let type_line = buf
        .syntax_spans_for_line(2)
        .expect("type line should exist");
    let number_line = buf
        .syntax_spans_for_line(3)
        .expect("number line should exist");
    let string_line = buf
        .syntax_spans_for_line(4)
        .expect("string line should exist");

    assert_spans_include_comment_style(&doc_comment);
    assert_spans_include_comment_style(&block_comment);
    assert_spans_include_style(&type_line, tag("keyword"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&string_line, tag("string"));
}
