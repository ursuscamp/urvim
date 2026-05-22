use super::*;

#[test]
fn test_perl_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-fixture", "pl", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let string_line = buf
        .syntax_spans_for_line(1)
        .expect("string line should exist");
    let array_line = buf
        .syntax_spans_for_line(2)
        .expect("array line should exist");
    let regex_line = buf
        .syntax_spans_for_line(3)
        .expect("regex line should exist");
    let number_line = buf
        .syntax_spans_for_line(4)
        .expect("number line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&string_line, tag("variable"));
    assert_spans_include_style(&array_line, tag("variable"));
    assert_spans_include_style(&regex_line, tag("string"));
    assert_spans_include_style(&number_line, tag("number"));
}

#[test]
fn test_perl_fixture_highlights_heredoc_markers() {
    let fixture = include_str!("fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-heredoc", "pl", fixture);

    let heredoc_opener = buf
        .syntax_spans_for_line(6)
        .expect("heredoc opener line should exist");
    let heredoc_body = buf
        .syntax_spans_for_line(7)
        .expect("heredoc body line should exist");
    let heredoc_end = buf
        .syntax_spans_for_line(8)
        .expect("heredoc terminator line should exist");

    assert_spans_include_style(&heredoc_opener, tag("string.escape"));
    assert_spans_include_style(&heredoc_body, tag("string.heredoc"));
    assert_spans_include_style(&heredoc_end, tag("string.escape"));
}

#[test]
fn test_perl_heredoc_opener_keeps_trailing_semicolon_out_of_body_text() {
    let fixture = include_str!("fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-heredoc-opener", "pl", fixture);

    let opener = buf
        .syntax_spans_for_line(6)
        .expect("heredoc opener line should exist");
    let line = buf
        .line_at(6)
        .expect("heredoc opener line should exist")
        .to_text();
    let semicolon = line
        .rfind(';')
        .expect("heredoc opener semicolon should exist");

    assert!(opener.iter().any(|span| {
        span.start_byte <= semicolon
            && semicolon < span.end_byte
            && span.style == tag("string.escape")
    }));
    assert!(!opener.iter().any(|span| {
        span.start_byte <= semicolon
            && semicolon < span.end_byte
            && span.style == tag("string.heredoc")
    }));
}
