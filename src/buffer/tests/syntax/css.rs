use super::*;

#[test]
fn test_css_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/css.css");
    let mut buf = fixture_buffer("syntax-css-fixture", "css", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let at_rule = buf
        .syntax_spans_for_line(1)
        .expect("at-rule line should exist");
    let selector = buf
        .syntax_spans_for_line(2)
        .expect("selector line should exist");
    let property = buf
        .syntax_spans_for_line(3)
        .expect("property line should exist");
    let number_line = buf
        .syntax_spans_for_line(4)
        .expect("number line should exist");
    let string_line = buf
        .syntax_spans_for_line(5)
        .expect("string line should exist");

    assert_spans_include_style(&comment, tag("comment"));
    assert_spans_include_style(&at_rule, tag("keyword"));
    assert_spans_include_style(&selector, tag("type"));
    assert_spans_include_style(&selector, tag("punctuation"));
    assert_spans_include_style(&property, tag("variable.property"));
    assert_spans_include_style(&property, tag("constant"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&string_line, tag("string"));
}
