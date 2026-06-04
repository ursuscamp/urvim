use super::*;

#[test]
fn test_ocaml_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/ocaml.ml");
    let mut buf = fixture_buffer("syntax-ocaml-fixture", "ml", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let string_line = buf
        .syntax_spans_for_line(5)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(6)
        .expect("number line should exist");
    let constant_line = buf
        .syntax_spans_for_line(7)
        .expect("constant line should exist");
    let char_line = buf
        .syntax_spans_for_line(8)
        .expect("char line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&constant_line, tag("constant"));
    assert_spans_include_style(&char_line, tag("constant"));
}
