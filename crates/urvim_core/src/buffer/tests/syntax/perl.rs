use super::*;

#[test]
fn test_perl_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-fixture", "pl", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let string_line = buf
        .syntax_spans_for_line(11)
        .expect("string line should exist");
    let array_line = buf
        .syntax_spans_for_line(12)
        .expect("array line should exist");
    let regex_line = buf
        .syntax_spans_for_line(26)
        .expect("regex line should exist");
    let number_line = buf
        .syntax_spans_for_line(14)
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
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-heredoc", "pl", fixture);

    let heredoc_opener = buf
        .syntax_spans_for_line(32)
        .expect("heredoc opener line should exist");
    let heredoc_body = buf
        .syntax_spans_for_line(33)
        .expect("heredoc body line should exist");
    let heredoc_end = buf
        .syntax_spans_for_line(34)
        .expect("heredoc terminator line should exist");

    assert_spans_include_style(&heredoc_opener, tag("string.escape"));
    assert_spans_include_style(&heredoc_body, tag("string.heredoc"));
    assert_spans_include_style(&heredoc_end, tag("string.escape"));
}

#[test]
fn test_perl_heredoc_opener_keeps_trailing_semicolon_out_of_body_text() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-heredoc-opener", "pl", fixture);

    let opener = buf
        .syntax_spans_for_line(32)
        .expect("heredoc opener line should exist");
    let line = buf
        .line_at(32)
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

#[test]
fn test_perl_fixture_highlights_pod_comments() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-pod", "pl", fixture);

    assert_perl_exact_style(&mut buf, 2, "=pod", tag("comment"));
    assert_perl_exact_style(&mut buf, 3, "Multi-line comment", tag("comment"));
    assert_perl_exact_style(&mut buf, 4, "in Perl pod format", tag("comment"));
    assert_perl_exact_style(&mut buf, 5, "=cut", tag("comment"));
}

#[test]
fn test_perl_fixture_highlights_base_and_exponent_number_literals() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-number-literals", "pl", fixture);

    assert_perl_exact_style(&mut buf, 18, "0xFF", tag("number"));
    assert_perl_exact_style(&mut buf, 19, "0777", tag("number"));
    assert_perl_exact_style(&mut buf, 20, "0b1010_0011", tag("number"));
    assert_perl_exact_style(&mut buf, 21, "1.5e-2", tag("number"));
}

#[test]
fn test_perl_fixture_highlights_regex_literals_consistently() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/perl.pl");
    let mut buf = fixture_buffer("syntax-perl-regex-literals", "pl", fixture);

    assert_perl_exact_style(&mut buf, 26, "m/hello/", tag("string"));
    assert_perl_exact_style(&mut buf, 27, "/Ada/", tag("string"));
    assert_perl_exact_style(&mut buf, 28, "s/Ada/Grace/r", tag("string"));
    assert_perl_exact_style(&mut buf, 29, "qr/pattern/", tag("string"));
    assert_perl_exact_style(&mut buf, 30, "m/world/", tag("string"));
}

fn assert_perl_exact_style(buf: &mut Buffer, line_index: usize, fragment: &str, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();

    assert_spans_include_exact_style(&spans, line.as_str(), fragment, style);
}
