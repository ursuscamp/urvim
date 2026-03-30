use super::*;

#[test]
fn test_fish_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/fish.fish");
    let mut buf = fixture_buffer("syntax-fish-fixture", "fish", fixture);

    let function_line = buf
        .syntax_spans_for_line(3)
        .expect("function line should exist");
    let declaration_line = buf
        .syntax_spans_for_line(4)
        .expect("declaration line should exist");
    let expansion_line = buf
        .syntax_spans_for_line(7)
        .expect("variable expansion line should exist");
    let command_substitution = buf
        .syntax_spans_for_line(11)
        .expect("command substitution line should exist");
    let math_line = buf
        .syntax_spans_for_line(12)
        .expect("math line should exist");

    assert_spans_include_style(&function_line, tag("keyword"));
    assert_spans_include_style(&declaration_line, tag("type"));
    assert_spans_include_style(&declaration_line, tag("string"));
    assert_spans_include_style(&expansion_line, tag("variable"));
    assert_spans_include_style(&expansion_line, tag("string"));
    assert_spans_include_style(&command_substitution, tag("punctuation"));
    assert_spans_include_style(&command_substitution, tag("type"));
    assert_spans_include_style(&command_substitution, tag("variable"));
    assert_spans_include_style(&math_line, tag("type"));
    assert_spans_include_style(&math_line, tag("number"));
}

#[test]
fn test_fish_shebang_resolves_to_fish() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-fish-shebang", "txt").as_path())
        .unwrap();
    let buf = Buffer::from_str_with_path("#!/usr/bin/env fish\nfunction greet", path);

    assert_eq!(buf.syntax_name(), "fish");
}
