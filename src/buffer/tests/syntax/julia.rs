use super::*;

#[test]
fn test_julia_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/julia.jl");
    let mut buf = fixture_buffer("syntax-julia-fixture", "jl", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let module_line = buf
        .syntax_spans_for_line(7)
        .expect("module line should exist");
    let macro_line = buf
        .syntax_spans_for_line(9)
        .expect("macro line should exist");
    let string_line = buf
        .syntax_spans_for_line(28)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(19)
        .expect("number line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&module_line, tag("keyword"));
    assert_spans_include_style(&macro_line, tag("function.macro"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
}
