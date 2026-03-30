use super::*;

#[test]
fn test_html_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/html.html");
    let mut buf = fixture_buffer("syntax-html-fixture", "html", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let doctype = buf
        .syntax_spans_for_line(1)
        .expect("doctype line should exist");
    let attribute_line = buf
        .syntax_spans_for_line(5)
        .expect("attribute line should exist");
    let script_line = buf
        .syntax_spans_for_line(7)
        .expect("script body line should exist");
    let style_line = buf
        .syntax_spans_for_line(10)
        .expect("style body line should exist");
    let entity_line = buf
        .syntax_spans_for_line(4)
        .expect("entity line should exist");
    let attribute_text = buf.line_at(5).expect("attribute line text should exist");
    let entity_start = attribute_text
        .find("&amp;")
        .expect("attribute entity should exist");
    let entity_end = entity_start + "&amp;".len();

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&doctype, tag("keyword"));
    assert_spans_include_style(&attribute_line, tag("markup.tag"));
    assert_spans_include_style(&attribute_line, tag("variable.property"));
    assert_spans_include_style(&attribute_line, tag("string"));
    assert!(attribute_line.iter().any(|span| {
        span.start_byte <= entity_start
            && span.end_byte >= entity_end
            && span.style == tag("constant")
    }));
    assert!(attribute_line.iter().any(|span| {
        span.start_byte <= entity_start
            && span.end_byte == entity_start
            && span.style == tag("string")
    }));
    assert!(
        attribute_line
            .iter()
            .any(|span| { span.start_byte == entity_end && span.style == tag("string") })
    );
    assert_spans_include_style(&script_line, tag("keyword"));
    assert_spans_include_style(&script_line, tag("number"));
    assert_spans_include_style(&script_line, tag("operator"));
    assert_spans_include_style(&style_line, tag("type"));
    assert_spans_include_style(&style_line, tag("variable.property"));
    assert_spans_include_style(&style_line, tag("constant"));
    assert_spans_include_style(&entity_line, tag("constant"));
}
