use super::*;

#[test]
fn test_cmake_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/CMakeLists.txt");
    let mut buf = named_fixture_buffer("cmake", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let keyword = buf
        .syntax_spans_for_line(1)
        .expect("keyword line should exist");
    let variables = buf
        .syntax_spans_for_line(4)
        .expect("variable line should exist");
    let bracket = buf
        .syntax_spans_for_line(5)
        .expect("bracket line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&keyword, tag("keyword"));
    assert_spans_include_style(&keyword, tag("number"));
    assert!(
        !keyword.iter().any(|span| span.style == tag("constant")),
        "VERSION should not be partially highlighted as a constant"
    );
    assert_spans_include_style(&variables, tag("string"));
    assert_spans_include_style(&variables, tag("variable"));
    assert_spans_include_style(&bracket, tag("string"));
}
