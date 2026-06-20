use super::*;

#[test]
fn test_zsh_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/zsh.zsh");
    let mut buf = fixture_buffer("syntax-zsh-fixture", "zsh", fixture);

    let option_line = buf
        .syntax_spans_for_line(4)
        .expect("option line should exist");
    let local_line = buf
        .syntax_spans_for_line(5)
        .expect("local line should exist");
    let parameter_line = buf
        .syntax_spans_for_line(7)
        .expect("parameter line should exist");
    let glob_line = buf
        .syntax_spans_for_line(8)
        .expect("glob line should exist");
    let array_line = buf
        .syntax_spans_for_line(9)
        .expect("array line should exist");

    assert_spans_include_style(&option_line, tag("type"));
    assert_spans_include_style(&local_line, tag("type"));
    assert_spans_include_style(&parameter_line, tag("string"));
    assert_spans_include_style(&parameter_line, tag("variable"));
    assert_spans_include_style(&glob_line, tag("punctuation"));
    assert_spans_include_style(&array_line, tag("type"));
    assert_spans_include_style(&array_line, tag("string"));
}

#[test]
fn test_zsh_shebang_resolves_to_zsh() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-zsh-shebang", "txt").as_path()).unwrap();
    let buf = Buffer::from_str_with_path("#!/usr/bin/env zsh\nsetopt localoptions", path);

    assert_eq!(buf.syntax_name(), "zsh");
}
