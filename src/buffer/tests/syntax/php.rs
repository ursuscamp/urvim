use super::*;

#[test]
fn test_php_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/php.php");
    let mut buf = fixture_buffer("syntax-php-fixture", "php", fixture);

    let comment = buf
        .syntax_spans_for_line(1)
        .expect("comment line should exist");
    let class_line = buf
        .syntax_spans_for_line(2)
        .expect("class line should exist");
    let prop_line = buf
        .syntax_spans_for_line(3)
        .expect("property line should exist");
    let number_line = buf
        .syntax_spans_for_line(4)
        .expect("number line should exist");
    let function_line = buf
        .syntax_spans_for_line(5)
        .expect("function line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&class_line, tag("keyword"));
    assert_spans_include_style(&prop_line, tag("variable"));
    assert_spans_include_style(&prop_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&function_line, tag("keyword"));
}

#[test]
fn test_php_fixture_highlights_heredoc_markers() {
    let fixture = include_str!("fixtures/php.php");
    let mut buf = fixture_buffer("syntax-php-heredoc", "php", fixture);

    let heredoc_opener = buf
        .syntax_spans_for_line(10)
        .expect("heredoc opener line should exist");
    let heredoc_body = buf
        .syntax_spans_for_line(11)
        .expect("heredoc body line should exist");
    let heredoc_end = buf
        .syntax_spans_for_line(12)
        .expect("heredoc terminator line should exist");

    assert_spans_include_style(&heredoc_opener, tag("string.escape"));
    assert_spans_include_style(&heredoc_body, tag("string.heredoc"));
    assert_spans_include_style(&heredoc_end, tag("string.escape"));
    assert_spans_include_style(&heredoc_end, tag("punctuation"));
}
