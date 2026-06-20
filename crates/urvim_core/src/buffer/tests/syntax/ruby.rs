use super::*;

#[test]
fn test_ruby_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/ruby.rb");
    let mut buf = fixture_buffer("syntax-ruby-fixture", "rb", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let class_line = buf
        .syntax_spans_for_line(7)
        .expect("class line should exist");
    let string_line = buf
        .syntax_spans_for_line(17)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(18)
        .expect("number line should exist");
    let symbol_line = buf
        .syntax_spans_for_line(28)
        .expect("symbol line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&class_line, tag("keyword"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&symbol_line, tag("constant"));
}

#[test]
fn test_ruby_fixture_highlights_heredoc_markers() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/ruby.rb");
    let mut buf = fixture_buffer("syntax-ruby-heredoc", "rb", fixture);

    let heredoc_opener = buf
        .syntax_spans_for_line(105)
        .expect("heredoc opener line should exist");
    let heredoc_body = buf
        .syntax_spans_for_line(106)
        .expect("heredoc body line should exist");
    let heredoc_end = buf
        .syntax_spans_for_line(108)
        .expect("heredoc terminator line should exist");

    assert_spans_include_style(&heredoc_opener, tag("string.escape"));
    assert_spans_include_style(&heredoc_body, tag("string.heredoc"));
    assert_spans_include_style(&heredoc_end, tag("string.escape"));
}

#[test]
fn test_ruby_fixture_highlights_multiline_comments() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/ruby.rb");
    let mut buf = fixture_buffer("syntax-ruby-block-comment", "rb", fixture);

    assert_ruby_exact_style(&mut buf, 2, "=begin", tag("comment"));
    assert_ruby_exact_style(&mut buf, 3, "Multi-line comment", tag("comment"));
    assert_ruby_exact_style(&mut buf, 4, "in Ruby", tag("comment"));
    assert_ruby_exact_style(&mut buf, 5, "=end", tag("comment"));
}

#[test]
fn test_ruby_fixture_highlights_base_and_exponent_number_literals() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/ruby.rb");
    let mut buf = fixture_buffer("syntax-ruby-number-literals", "rb", fixture);

    assert_ruby_exact_style(&mut buf, 21, "0xFF", tag("number"));
    assert_ruby_exact_style(&mut buf, 22, "0o77", tag("number"));
    assert_ruby_exact_style(&mut buf, 23, "0b1010_0011", tag("number"));
    assert_ruby_exact_style(&mut buf, 24, "1.5e-2", tag("number"));
}

#[test]
fn test_ruby_fixture_highlights_question_mark_character_literals() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/ruby.rb");
    let mut buf = fixture_buffer("syntax-ruby-character-literal", "rb", fixture);

    assert_ruby_exact_style(&mut buf, 25, "?x", tag("string"));
}

#[test]
fn test_ruby_question_mark_still_highlights_as_ternary_operator() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-ruby-ternary", "rb").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("value = ok ? true : false", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_exact_style(&spans, "value = ok ? true : false", "?", tag("operator"));
}

fn assert_ruby_exact_style(buf: &mut Buffer, line_index: usize, fragment: &str, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();

    assert_spans_include_exact_style(&spans, line.as_str(), fragment, style);
}
