use super::*;

fn assert_css_folds(source: &str, expected: &[(usize, usize)]) {
    let mut buf = fixture_buffer("syntax-css-folds", "css", source);
    let actual: Vec<(usize, usize)> = buf
        .syntax_fold_regions()
        .iter()
        .map(|region| (region.start_line, region.end_line))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn test_css_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/css.css");
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

#[test]
fn test_css_brace_and_paren_folds_ignore_strings_and_comments() {
    assert_css_folds(
        "/* { */\n@media screen {\n  .card {\n    background: linear-gradient(\n      red,\n      blue\n    );\n  }\n}\n",
        &[(2, 7), (1, 8)],
    );
}

#[test]
fn test_css_same_line_blocks_are_discarded() {
    assert_css_folds(".card { color: red; }\n", &[]);
}
