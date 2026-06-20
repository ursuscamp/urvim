use super::*;

#[test]
fn test_powershell_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/powershell.ps1");
    let mut buf = fixture_buffer("syntax-powershell-fixture", "ps1", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let function_line = buf
        .syntax_spans_for_line(7)
        .expect("function line should exist");
    let variable_line = buf
        .syntax_spans_for_line(10)
        .expect("variable line should exist");
    let here_string = buf
        .syntax_spans_for_line(21)
        .expect("here-string line should exist");
    let constant_line = buf
        .syntax_spans_for_line(14)
        .expect("constant line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&function_line, tag("keyword"));
    assert_spans_include_style(&variable_line, tag("variable"));
    assert_spans_include_style(&here_string, tag("string"));
    assert_spans_include_style(&constant_line, tag("constant"));
}

#[test]
fn test_powershell_fixture_highlights_block_comment_contents() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/powershell.ps1");
    let mut buf = fixture_buffer("syntax-powershell-block-comment", "ps1", fixture);

    assert_powershell_exact_style(&mut buf, 2, "<#", tag("comment"));
    assert_powershell_exact_style(&mut buf, 3, "  Multi-line block comment", tag("comment"));
    assert_powershell_exact_style(&mut buf, 4, "  in PowerShell", tag("comment"));
    assert_powershell_exact_style(&mut buf, 5, "#>", tag("comment"));
}

#[test]
fn test_powershell_variable_names_do_not_split_as_constants() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/powershell.ps1");
    let mut buf = fixture_buffer("syntax-powershell-constant-boundary", "ps1", fixture);

    assert_powershell_exact_style(&mut buf, 15, "$falseFlag", tag("variable"));
    assert_powershell_nested_exact_style(&mut buf, 15, "= $false", "$false", tag("constant"));
    assert_powershell_exact_style(&mut buf, 16, "$nullVal", tag("variable"));
    assert_powershell_nested_exact_style(&mut buf, 16, "= $null", "$null", tag("constant"));
}

#[test]
fn test_powershell_fixture_highlights_base_number_literals() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/powershell.ps1");
    let mut buf = fixture_buffer("syntax-powershell-number-literals", "ps1", fixture);

    assert_powershell_exact_style(&mut buf, 18, "0xFF", tag("number"));
    assert_powershell_exact_style(&mut buf, 19, "0b1010_0011", tag("number"));
}

fn assert_powershell_exact_style(buf: &mut Buffer, line_index: usize, fragment: &str, style: Tag) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();

    assert_spans_include_exact_style(&spans, line.as_str(), fragment, style);
}

fn assert_powershell_nested_exact_style(
    buf: &mut Buffer,
    line_index: usize,
    containing_fragment: &str,
    fragment: &str,
    style: Tag,
) {
    let spans = buf
        .syntax_spans_for_line(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"));
    let line = buf
        .line_at(line_index)
        .unwrap_or_else(|| panic!("line {line_index} should exist"))
        .to_string();
    let containing_start = line
        .find(containing_fragment)
        .unwrap_or_else(|| panic!("expected line to contain fragment {containing_fragment:?}"));
    let start = line[containing_start..]
        .find(fragment)
        .map(|offset| containing_start + offset)
        .unwrap_or_else(|| panic!("expected nested fragment {fragment:?}"));
    let end = start + fragment.len();

    assert!(
        spans
            .iter()
            .any(|span| span.start_byte == start && span.end_byte == end && span.style == style),
        "expected nested fragment {fragment:?} at {start}..{end} to use style {style:?}"
    );
}
