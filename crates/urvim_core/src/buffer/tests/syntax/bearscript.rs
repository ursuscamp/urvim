use super::*;

#[test]
fn test_bearscript_fixture_uses_lexer_grammar() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/bearscript.bear");
    let mut buffer = fixture_buffer("syntax-bearscript-fixture", "bear", fixture);

    let comment_line = line_containing(&buffer, "BearScript syntax fixture");
    let import_line = line_containing(&buffer, "import tools");
    let generator_line = line_containing(&buffer, "gen count");
    let range_line = line_containing(&buffer, "0..limit");
    let interpolation_line = line_containing(&buffer, "helpers.format");
    let call_line = line_containing(&buffer, "let output = print");
    let comment = buffer
        .syntax_spans_for_line(comment_line)
        .expect("comment line should exist");
    let import = buffer
        .syntax_spans_for_line(import_line)
        .expect("import line should exist");
    let generator = buffer
        .syntax_spans_for_line(generator_line)
        .expect("generator line should exist");
    let range = buffer
        .syntax_spans_for_line(range_line)
        .expect("range line should exist");
    let interpolation = buffer
        .syntax_spans_for_line(interpolation_line)
        .expect("interpolation line should exist");
    let call = buffer
        .syntax_spans_for_line(call_line)
        .expect("call line should exist");
    let import_text = buffer
        .line_at(import_line)
        .expect("import line should exist")
        .to_string();
    let generator_text = buffer
        .line_at(generator_line)
        .expect("generator line should exist")
        .to_string();
    let range_text = buffer
        .line_at(range_line)
        .expect("range line should exist")
        .to_string();
    let interpolation_text = buffer
        .line_at(interpolation_line)
        .expect("interpolation line should exist")
        .to_string();
    let call_text = buffer
        .line_at(call_line)
        .expect("call line should exist")
        .to_string();

    assert_spans_include_comment_style(&comment);
    assert_spans_include_exact_style(&import, &import_text, "import", tag("keyword"));
    assert_spans_include_exact_style(&generator, &generator_text, "gen", tag("keyword"));
    assert_spans_include_exact_style(&range, &range_text, "..", tag("punctuation"));
    assert_spans_include_exact_style(
        &interpolation,
        &interpolation_text,
        "format",
        tag("variable.property"),
    );
    assert_spans_include_exact_style(&call, &call_text, "print", tag("function"));
    assert_spans_include_style(&interpolation, tag("string"));
}
