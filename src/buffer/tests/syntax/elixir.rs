use super::*;

#[test]
fn test_elixir_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/elixir.ex");
    let mut buf = fixture_buffer("syntax-elixir-fixture", "ex", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let module_line = buf
        .syntax_spans_for_line(1)
        .expect("module line should exist");
    let attr_line = buf
        .syntax_spans_for_line(2)
        .expect("attribute line should exist");
    let string_line = buf
        .syntax_spans_for_line(4)
        .expect("string line should exist");
    let atom_line = buf
        .syntax_spans_for_line(7)
        .expect("atom line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&module_line, tag("keyword"));
    assert_spans_include_style(&module_line, tag("type"));
    assert_spans_include_style(&attr_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&atom_line, tag("constant"));
}
