use super::*;

#[test]
fn test_ruby_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/ruby.rb");
    let mut buf = fixture_buffer("syntax-ruby-fixture", "rb", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let class_line = buf
        .syntax_spans_for_line(1)
        .expect("class line should exist");
    let string_line = buf
        .syntax_spans_for_line(3)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(4)
        .expect("number line should exist");
    let symbol_line = buf
        .syntax_spans_for_line(6)
        .expect("symbol line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&class_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&symbol_line, tag("constant"));
}

#[test]
fn test_ruby_fixture_highlights_heredoc_markers() {
    let fixture = include_str!("fixtures/ruby.rb");
    let mut buf = fixture_buffer("syntax-ruby-heredoc", "rb", fixture);

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
}
