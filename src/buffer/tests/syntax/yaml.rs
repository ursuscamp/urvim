use super::*;

#[test]
fn test_yaml_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/yaml.yaml");
    let mut buf = fixture_buffer("syntax-yaml-fixture", "yaml", fixture);

    let directive = buf
        .syntax_spans_for_line(1)
        .expect("directive line should exist");
    let key_line = buf.syntax_spans_for_line(2).expect("key line should exist");
    let string_line = buf
        .syntax_spans_for_line(3)
        .expect("string line should exist");
    let block_header = buf
        .syntax_spans_for_line(4)
        .expect("block header line should exist");
    let block_body = buf
        .syntax_spans_for_line(5)
        .expect("block body line should exist");
    let anchor_line = buf
        .syntax_spans_for_line(8)
        .expect("anchor line should exist");
    let alias_line = buf
        .syntax_spans_for_line(10)
        .expect("alias line should exist");
    let tag_line = buf
        .syntax_spans_for_line(11)
        .expect("tag line should exist");

    assert_spans_include_style(&directive, tag("keyword"));
    assert_spans_include_style(&key_line, tag("variable.property"));
    assert_spans_include_style(&key_line, tag("string"));
    assert_spans_include_style(&string_line, tag("string"));
    assert_spans_include_style(&block_header, tag("string"));
    assert_spans_include_style(&block_body, tag("string"));
    assert_spans_include_style(&anchor_line, tag("variable"));
    assert_spans_include_style(&alias_line, tag("variable"));
    assert_spans_include_style(&tag_line, tag("variable"));
}
