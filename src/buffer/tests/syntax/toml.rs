use super::*;

#[test]
fn test_toml_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/toml.toml");
    let mut buf = fixture_buffer("syntax-toml-fixture", "toml", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let string_line = buf
        .syntax_spans_for_line(1)
        .expect("string line should exist");
    let number_line = buf
        .syntax_spans_for_line(2)
        .expect("number line should exist");
    let bool_line = buf
        .syntax_spans_for_line(3)
        .expect("bool line should exist");
    let table_line = buf
        .syntax_spans_for_line(5)
        .expect("table line should exist");
    let array_line = buf
        .syntax_spans_for_line(11)
        .expect("array line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&string_line, tag("operator"));
    assert_spans_include_style(&number_line, tag("number"));
    assert_spans_include_style(&number_line, tag("operator"));
    assert_spans_include_style(&bool_line, tag("constant"));
    assert_spans_include_style(&bool_line, tag("operator"));
    assert_spans_include_style(&table_line, tag("keyword"));
    assert_spans_include_style(&array_line, tag("number"));
    assert_spans_include_style(&array_line, tag("punctuation"));
}

#[test]
fn test_toml_fixture_highlights_tables_and_extended_numbers() {
    let fixture = include_str!("fixtures/toml.toml");
    let mut buf = fixture_buffer("syntax-toml-extended", "toml", fixture);

    let dotted_key = buf
        .syntax_spans_for_line(17)
        .expect("dotted key line should exist");
    let table = buf
        .syntax_spans_for_line(19)
        .expect("table line should exist");
    let base_numbers = buf
        .syntax_spans_for_line(20)
        .expect("base number line should exist");
    let inline_table = buf
        .syntax_spans_for_line(21)
        .expect("inline table line should exist");
    let array_of_tables = buf
        .syntax_spans_for_line(23)
        .expect("array of tables line should exist");

    assert_spans_include_style(&dotted_key, tag("variable"));
    assert_spans_include_style(&dotted_key, tag("operator"));
    assert_spans_include_style(&table, tag("keyword"));
    assert_spans_include_style(&base_numbers, tag("number"));
    assert_spans_include_style(&inline_table, tag("number"));
    assert_spans_include_style(&inline_table, tag("constant"));
    assert_spans_include_style(&array_of_tables, tag("keyword"));
}

#[test]
fn test_toml_constants_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-toml-constant", "toml").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("flag = true", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("constant"));
}

#[test]
fn test_toml_datetimes_use_number_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-toml-datetime", "toml").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path(
        "offset = 1979-05-27T07:32:00Z\nlocal = 1979-05-27 07:32:00\ndate = 1979-05-27\ntime = 07:32:00",
        path,
    );

    for line in 0..4 {
        let spans = buf.syntax_spans_for_line(line).expect("line should exist");
        assert_spans_include_style(&spans, tag("number"));
    }
}

#[test]
fn test_toml_literal_strings_remain_plain() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-toml-literal", "toml").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("raw = 'line \\n two'", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert!(!spans.iter().any(|span| span.style == tag("punctuation")));
}

#[test]
fn test_toml_quote_inside_double_quoted_string() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-toml-quote", "toml").as_path())
            .unwrap();
    let mut buf =
        Buffer::from_str_with_path("name = \"it's value\"\nnext = 42", path);

    let line0 = buf.syntax_spans_for_line(0).expect("line 0 should exist");
    assert_spans_include_style(&line0, tag("string"));
    assert_spans_include_style(&line0, tag("operator"));

    let line1 = buf.syntax_spans_for_line(1).expect("line 1 should exist");
    assert_spans_include_style(&line1, tag("number"));
}
