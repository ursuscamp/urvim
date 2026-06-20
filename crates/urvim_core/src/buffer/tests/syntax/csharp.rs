use super::*;

#[test]
fn test_csharp_fixture_uses_grammar_rules() {
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/csharp.cs");
    let mut buf = fixture_buffer("syntax-csharp-fixture", "cs", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let attribute = buf
        .syntax_spans_for_line(5)
        .expect("attribute line should exist");
    let class_line = buf
        .syntax_spans_for_line(6)
        .expect("class line should exist");
    let string_line = buf
        .syntax_spans_for_line(11)
        .expect("string line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&attribute, tag("keyword"));
    assert_spans_include_style(&class_line, tag("keyword"));
    assert_spans_include_style(&class_line, tag("type"));
    assert_spans_include_style(&string_line, tag("string"));
}

#[test]
fn test_csharp_function_call_highlights_function_name() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-csharp-function", "cs").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("Render(Name);", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function"));
}
