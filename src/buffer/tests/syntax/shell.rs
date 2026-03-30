use super::*;

#[test]
fn test_shell_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/shell.sh");
    let mut buf = fixture_buffer("syntax-shell-fixture", "sh", fixture);

    let comment = buf
        .syntax_spans_for_line(1)
        .expect("comment line should exist");
    let function_line = buf
        .syntax_spans_for_line(3)
        .expect("function line should exist");
    let assignment_line = buf
        .syntax_spans_for_line(4)
        .expect("assignment line should exist");
    let keyword_line = buf
        .syntax_spans_for_line(9)
        .expect("keyword line should exist");
    let keyword_line_text = buf
        .line_at(9)
        .expect("keyword line should exist")
        .to_string();

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&function_line, tag("punctuation"));
    assert_spans_include_style(&assignment_line, tag("type"));
    assert_spans_include_style(&assignment_line, tag("operator"));
    assert_spans_include_style(&assignment_line, tag("string"));
    assert_spans_include_style(&keyword_line, tag("keyword"));
    assert_spans_include_style(&keyword_line, tag("constant"));
    assert_spans_include_style(&keyword_line, tag("punctuation"));
    assert_spans_include_exact_style(
        &keyword_line,
        keyword_line_text.as_str(),
        "if",
        tag("keyword"),
    );
}

#[test]
fn test_shell_fixture_highlights_substitutions_and_heredoc_marker() {
    let fixture = include_str!("fixtures/shell.sh");
    let mut buf = fixture_buffer("syntax-shell-extended", "sh", fixture);

    let parameter = buf
        .syntax_spans_for_line(21)
        .expect("parameter expansion line should exist");
    let command = buf
        .syntax_spans_for_line(22)
        .expect("command substitution line should exist");
    let arithmetic = buf
        .syntax_spans_for_line(23)
        .expect("arithmetic substitution line should exist");
    let heredoc = buf
        .syntax_spans_for_line(26)
        .expect("heredoc opener line should exist");
    let heredoc_body = buf
        .syntax_spans_for_line(27)
        .expect("heredoc body line should exist");
    let heredoc_end = buf
        .syntax_spans_for_line(28)
        .expect("heredoc terminator line should exist");

    assert_spans_include_style(&parameter, tag("string"));
    assert_spans_include_style(&parameter, tag("variable"));
    assert_spans_include_style(&command, tag("string"));
    assert_spans_include_style(&command, tag("punctuation"));
    assert_spans_include_style(&arithmetic, tag("string"));
    assert_spans_include_style(&arithmetic, tag("punctuation"));
    assert_spans_include_style(&heredoc, tag("string.escape"));
    assert_spans_include_style(&heredoc_body, tag("string.heredoc"));
    assert_spans_include_style(&heredoc_end, tag("string.escape"));
}

#[test]
fn test_shell_arithmetic_substitution_keeps_both_parens_punctuation() {
    let fixture = include_str!("fixtures/shell.sh");
    let mut buf = fixture_buffer("syntax-shell-arithmetic", "sh", fixture);

    let spans = buf
        .syntax_spans_for_line(23)
        .expect("arithmetic line should exist");
    assert_spans_include_style(&spans, tag("punctuation"));
    assert!(spans.iter().any(|span| {
        span.start_byte <= 6 && span.end_byte >= 9 && span.style == tag("punctuation")
    }));
}

#[test]
fn test_shell_fixture_does_not_strip_tabs_in_heredoc_delimiters() {
    let fixture = include_str!("fixtures/shell.sh");
    let mut buf = fixture_buffer("syntax-shell-tabbed-heredoc", "sh", fixture);

    let heredoc_opener = buf
        .syntax_spans_for_line(30)
        .expect("tabbed heredoc opener line should exist");
    let heredoc_body = buf
        .syntax_spans_for_line(31)
        .expect("tabbed heredoc body line should exist");
    let tabbed_terminator = buf
        .syntax_spans_for_line(32)
        .expect("tabbed heredoc terminator line should exist");
    let real_terminator = buf
        .syntax_spans_for_line(33)
        .expect("real heredoc terminator line should exist");

    assert_spans_include_style(&heredoc_opener, tag("string.escape"));
    assert_spans_include_style(&heredoc_body, tag("string.heredoc"));
    assert_spans_include_style(&tabbed_terminator, tag("string.heredoc"));
    assert_spans_include_style(&real_terminator, tag("string.escape"));
}

#[test]
fn test_shell_single_quotes_remain_plain() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-shell-single", "sh").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("msg='line \\n two'", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert!(!spans.iter().any(|span| span.style == tag("punctuation")));
}

#[test]
fn test_shell_constants_use_constant_rules() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-shell-constant", "sh").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("if true; then echo ok; fi", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("constant"));
}

#[test]
fn test_shell_types_use_type_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-shell-type", "sh").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("local name=\"Ada\"; export PATH=/usr/bin", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("type"));
}
