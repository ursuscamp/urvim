use super::*;

fn assert_html_folds(source: &str, expected: &[(usize, usize)]) {
    let mut buf = fixture_buffer("syntax-html-folds", "html", source);
    let actual: Vec<(usize, usize)> = buf
        .syntax_fold_regions()
        .iter()
        .map(|region| (region.start_line, region.end_line))
        .collect();
    assert_eq!(actual, expected);
}

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
    let script_close = buf
        .syntax_spans_for_line(8)
        .expect("script close line should exist");
    let style_line = buf
        .syntax_spans_for_line(10)
        .expect("style body line should exist");
    let style_close = buf
        .syntax_spans_for_line(11)
        .expect("style close line should exist");
    let entity_line = buf
        .syntax_spans_for_line(4)
        .expect("entity line should exist");
    let comment_text = buf
        .line_at(0)
        .expect("comment line text should exist")
        .to_text();

    assert_spans_include_comment_style(&comment);
    assert_spans_include_exact_style(
        &comment,
        &comment_text,
        " HTML syntax fixture ",
        tag("comment"),
    );
    assert_spans_include_style(&doctype, tag("keyword"));
    assert_spans_include_style(&attribute_line, tag("markup.tag"));
    assert_spans_include_style(&attribute_line, tag("variable.property"));
    assert_spans_include_style(&attribute_line, tag("string"));
    assert_exact_spans(
        &entity_line,
        &[
            (0, 1, tag("punctuation")),
            (1, 2, tag("markup.tag")),
            (2, 3, tag("punctuation")),
            (3, 8, tag("constant")),
            (8, 9, tag("punctuation")),
            (9, 10, tag("punctuation")),
            (10, 11, tag("markup.tag")),
            (11, 12, tag("punctuation")),
        ],
    );
    assert_exact_spans(
        &attribute_line,
        &[
            (0, 1, tag("punctuation")),
            (1, 4, tag("markup.tag")),
            (5, 8, tag("variable.property")),
            (8, 9, tag("punctuation")),
            (9, 10, tag("string")),
            (10, 19, tag("string")),
            (19, 20, tag("string")),
            (21, 24, tag("variable.property")),
            (24, 25, tag("punctuation")),
            (25, 26, tag("string")),
            (26, 29, tag("string")),
            (29, 34, tag("constant")),
            (34, 38, tag("string")),
            (38, 39, tag("string")),
            (40, 41, tag("punctuation")),
            (41, 42, tag("punctuation")),
        ],
    );
    assert_spans_include_style(&script_line, tag("keyword"));
    assert_spans_include_style(&script_line, tag("number"));
    assert_spans_include_style(&script_line, tag("operator"));
    assert_exact_spans(
        &script_line,
        &[
            (0, 5, tag("keyword")),
            (6, 11, tag("variable")),
            (12, 13, tag("punctuation")),
            (14, 15, tag("number")),
            (16, 17, tag("operator")),
            (18, 19, tag("number")),
            (19, 20, tag("punctuation")),
        ],
    );
    assert_html_closing_tag_spans(&script_close, "</script>", "script");
    assert_spans_include_style(&style_line, tag("type"));
    assert_spans_include_style(&style_line, tag("variable.property"));
    assert_spans_include_style(&style_line, tag("constant"));
    assert_html_closing_tag_spans(&style_close, "</style>", "style");
    assert_spans_include_style(&entity_line, tag("constant"));
}

fn assert_html_closing_tag_spans(
    spans: &[crate::buffer::syntax::SyntaxSpan],
    line: &str,
    name: &str,
) {
    assert_spans_include_exact_style(spans, line, "<", tag("punctuation"));
    assert_spans_include_exact_style(spans, line, "/", tag("punctuation"));
    assert_spans_include_exact_style(spans, line, name, tag("markup.tag"));
    assert_spans_include_exact_style(spans, line, ">", tag("punctuation"));
}

#[test]
fn test_html_multiline_comment_body_uses_comment_style() {
    let mut buf = fixture_buffer(
        "syntax-html-comment",
        "html",
        "<!-- start\nbody text\nend -->",
    );

    let opener = buf
        .syntax_spans_for_line(0)
        .expect("comment opener should tokenize");
    let body = buf
        .syntax_spans_for_line(1)
        .expect("comment body should tokenize");
    let closer = buf
        .syntax_spans_for_line(2)
        .expect("comment closer should tokenize");
    let opener_text = buf
        .line_at(0)
        .expect("comment opener text should exist")
        .to_text();
    let body_text = buf
        .line_at(1)
        .expect("comment body text should exist")
        .to_text();
    let closer_text = buf
        .line_at(2)
        .expect("comment closer text should exist")
        .to_text();

    assert_spans_include_exact_style(&opener, &opener_text, "<!--", tag("comment"));
    assert_spans_include_exact_style(&opener, &opener_text, " start", tag("comment"));
    assert_spans_include_exact_style(&body, &body_text, "body text", tag("comment"));
    assert_spans_include_exact_style(&closer, &closer_text, "end ", tag("comment"));
    assert_spans_include_exact_style(&closer, &closer_text, "-->", tag("comment"));
}

#[test]
fn test_html_tag_folds_nested_elements() {
    assert_html_folds(
        "<div>\n  <section>\n    text\n  </section>\n</div>\n",
        &[(1, 3), (0, 4)],
    );
}

#[test]
fn test_html_tag_folds_match_nearest_same_tag() {
    assert_html_folds(
        "<div>\n  <div>\n    text\n  </div>\n</div>\n",
        &[(1, 3), (0, 4)],
    );
}

#[test]
fn test_html_void_and_self_closing_tags_do_not_fold() {
    assert_html_folds(
        "<div>\n  <img src=\"x\">\n  <custom />\n  <custom\n    prop=\"x\"\n  />\n</div>\n",
        &[(0, 6)],
    );
}

#[test]
fn test_html_tags_in_attributes_and_comments_do_not_fold() {
    assert_html_folds(
        "<div data-x=\"<section>\">\n  <!-- <section> -->\n</div>\n",
        &[(0, 2)],
    );
}

#[test]
fn test_html_script_and_style_host_tags_fold() {
    assert_html_folds(
        "<html>\n<script>\nconst value = 1;\n</script>\n<style>\nbody { color: red; }\n</style>\n</html>\n",
        &[(1, 3), (4, 6), (0, 7)],
    );
}
