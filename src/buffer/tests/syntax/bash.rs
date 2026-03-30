use super::*;

#[test]
fn test_bash_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/bash.sh");
    let mut buf = fixture_buffer("syntax-bash-fixture", "bash", fixture);

    let array_line = buf
        .syntax_spans_for_line(4)
        .expect("array line should exist");
    let conditional_line = buf
        .syntax_spans_for_line(9)
        .expect("conditional line should exist");
    let arithmetic_line = buf
        .syntax_spans_for_line(13)
        .expect("arithmetic line should exist");
    let ansi_line = buf
        .syntax_spans_for_line(14)
        .expect("ansi-c quote line should exist");
    let heredoc_line = buf
        .syntax_spans_for_line(17)
        .expect("heredoc opener line should exist");
    let heredoc_body = buf
        .syntax_spans_for_line(18)
        .expect("heredoc body line should exist");
    let heredoc_end = buf
        .syntax_spans_for_line(19)
        .expect("heredoc terminator line should exist");

    assert_spans_include_style(&array_line, tag("type"));
    assert_spans_include_style(&array_line, tag("string"));
    assert_spans_include_style(&conditional_line, tag("keyword"));
    assert_spans_include_style(&conditional_line, tag("variable"));
    assert_spans_include_style(&conditional_line, tag("punctuation"));
    assert_spans_include_style(&arithmetic_line, tag("keyword"));
    assert_spans_include_style(&arithmetic_line, tag("number"));
    assert_spans_include_style(&ansi_line, tag("string"));
    assert_spans_include_style(&heredoc_line, tag("string.escape"));
    assert_spans_include_style(&heredoc_body, tag("string.heredoc"));
    assert_spans_include_style(&heredoc_end, tag("string.escape"));
}

#[test]
fn test_bash_typeset_is_not_split() {
    let fixture = include_str!("fixtures/bash.sh");
    let mut buf = fixture_buffer("syntax-bash-typeset", "bash", fixture);

    let line = buf
        .syntax_spans_for_line(6)
        .expect("typeset line should exist");
    let text = buf
        .line_at(6)
        .expect("typeset line should exist")
        .to_string();
    let start = text.find("typeset").expect("typeset token should exist");
    let end = start + "typeset".len();

    assert!(
        line.iter().any(|span| span.start_byte == start
            && span.end_byte == end
            && span.style == tag("type")),
        "expected a single span for `typeset`, got {line:?}"
    );
    assert!(
        !line
            .iter()
            .any(|span| span.start_byte > start && span.start_byte < end),
        "expected `typeset` not to be split, got {line:?}"
    );
}

#[test]
fn test_bash_parameter_expansion_in_double_quotes_is_not_string_text() {
    let fixture = include_str!("fixtures/bash.sh");
    let mut buf = fixture_buffer("syntax-bash-parameter-expansion", "bash", fixture);

    let spans = buf
        .syntax_spans_for_line(10)
        .expect("parameter expansion line should exist");
    let line = buf
        .line_at(10)
        .expect("parameter expansion line should exist")
        .to_string();
    let start = line
        .find("${targets[0]}")
        .expect("parameter expansion should exist");
    let end = start + "${targets[0]}".len();

    assert_spans_include_style(&spans, tag("variable"));
    assert!(spans.iter().any(|span| {
        span.start_byte <= start && span.end_byte >= end && span.style == tag("variable")
    }));
    assert!(!spans.iter().any(|span| {
        span.start_byte >= start && span.end_byte <= end && span.style == tag("string")
    }));
}

#[test]
fn test_bash_printf_format_string_highlights_formatting_characters() {
    let fixture = include_str!("fixtures/bash.sh");
    let mut buf = fixture_buffer("syntax-bash-printf-format", "bash", fixture);

    let spans = buf
        .syntax_spans_for_line(10)
        .expect("printf line should exist");
    let line = buf
        .line_at(10)
        .expect("printf line should exist")
        .to_string();
    let percent_start = line.find('%').expect("format percent should exist");
    let escape_start = line.find("\\n").expect("format escape should exist");

    assert_spans_include_style(&spans, tag("type"));
    assert!(spans.iter().any(|span| {
        span.start_byte <= percent_start
            && span.end_byte > percent_start
            && span.style == tag("string.interpolation")
    }));
    assert!(spans.iter().any(|span| {
        span.start_byte <= escape_start
            && span.end_byte >= escape_start + 2
            && span.style == tag("string.escape")
    }));
    assert!(spans.iter().any(|span| span.style == tag("string")));
}

#[test]
fn test_bash_shebang_resolves_to_bash() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-bash-shebang", "txt").as_path())
        .unwrap();
    let buf = Buffer::from_str_with_path("#!/usr/bin/env bash\n[[ -n $HOME ]]", path);

    assert_eq!(buf.syntax_name(), "bash");
}
