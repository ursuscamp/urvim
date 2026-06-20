use super::*;

fn assert_json_folds(source: &str, expected: &[(usize, usize)]) {
    let mut buf = fixture_buffer("syntax-json-folds", "json", source);
    let actual: Vec<(usize, usize)> = buf
        .syntax_fold_regions()
        .iter()
        .map(|region| (region.start_line, region.end_line))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn test_json_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/json.json");
    let mut buf = fixture_buffer("syntax-json-fixture", "json", fixture);

    let line_one = buf.syntax_spans_for_line(1).expect("line should exist");
    let line_three = buf.syntax_spans_for_line(3).expect("line should exist");
    let line_four = buf.syntax_spans_for_line(4).expect("line should exist");
    let line_five = buf.syntax_spans_for_line(5).expect("line should exist");

    assert_spans_include_style(&line_one, tag("string"));
    assert_spans_include_style(&line_one, tag("punctuation"));
    assert_spans_include_style(&line_three, tag("constant"));
    assert_spans_include_style(&line_three, tag("punctuation"));
    assert_spans_include_style(&line_four, tag("number"));
    assert_spans_include_style(&line_four, tag("punctuation"));
    assert_spans_include_style(&line_five, tag("punctuation"));
}

#[test]
fn test_json_fixture_rejects_identifier_like_text() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/json.json");
    let mut buf = fixture_buffer("syntax-json-extended", "json", fixture);

    let negative = buf
        .syntax_spans_for_line(12)
        .expect("negative number line should exist");
    let identifier = buf
        .syntax_spans_for_line(13)
        .expect("identifier-like line should exist");

    assert_spans_include_style(&negative, tag("number"));
    assert!(identifier.is_empty());
}

#[test]
fn test_json_strings_use_escape_regions() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-json-string", "json").as_path())
        .unwrap();
    let mut buf =
        Buffer::from_str_with_path("{\"key\": \"line 1\\nline 2\", \"enabled\": true}", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("constant"));
}

#[test]
fn test_json_object_and_array_folds_ignore_strings() {
    assert_json_folds(
        "{\n  \"text\": \"{\",\n  \"items\": [\n    {\n      \"value\": 1\n    }\n  ]\n}\n",
        &[(3, 5), (2, 6), (0, 7)],
    );
}

#[test]
fn test_json_same_line_structures_are_discarded() {
    assert_json_folds("{\"items\": [1, 2, 3]}\n", &[]);
}
