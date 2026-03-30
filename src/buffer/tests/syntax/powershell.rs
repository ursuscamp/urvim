use super::*;

#[test]
fn test_powershell_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/powershell.ps1");
    let mut buf = fixture_buffer("syntax-powershell-fixture", "ps1", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let function_line = buf
        .syntax_spans_for_line(1)
        .expect("function line should exist");
    let variable_line = buf
        .syntax_spans_for_line(2)
        .expect("variable line should exist");
    let here_string = buf
        .syntax_spans_for_line(4)
        .expect("here-string line should exist");
    let constant_line = buf
        .syntax_spans_for_line(7)
        .expect("constant line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&function_line, tag("keyword"));
    assert_spans_include_style(&variable_line, tag("variable"));
    assert_spans_include_style(&here_string, tag("string"));
    assert_spans_include_style(&constant_line, tag("constant"));
}
