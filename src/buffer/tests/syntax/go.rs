use super::*;

#[test]
fn test_go_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/go.go");
    let mut buf = fixture_buffer("syntax-go-fixture", "go", fixture);

    let package_line = buf
        .syntax_spans_for_line(1)
        .expect("package line should exist");
    let raw_string = buf
        .syntax_spans_for_line(6)
        .expect("raw string line should exist");
    let raw_string_body = buf
        .syntax_spans_for_line(7)
        .expect("raw string body line should exist");
    let rune = buf
        .syntax_spans_for_line(8)
        .expect("rune line should exist");
    let number_line = buf
        .syntax_spans_for_line(9)
        .expect("number line should exist");
    let bool_line = buf
        .syntax_spans_for_line(10)
        .expect("bool line should exist");
    let call_line = buf
        .syntax_spans_for_line(12)
        .expect("call line should exist");

    assert_spans_include_style(&package_line, tag("keyword"));
    assert_spans_include_style(&raw_string, tag("string"));
    assert_spans_include_style(&raw_string_body, tag("string"));
    assert_spans_include_style(&rune, tag("constant"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&bool_line, tag("keyword"));
    assert_spans_include_style(&bool_line, tag("constant"));
    assert_spans_include_style(&call_line, tag("function"));
}
