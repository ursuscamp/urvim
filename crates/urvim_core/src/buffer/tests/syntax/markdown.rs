use super::*;

fn markdown_buffer(name: &str, text: &str) -> Buffer {
    let path = AbsolutePath::from_path(temp_path_with_ext(name, "md").as_path()).unwrap();
    Buffer::from_str_with_path(text, path)
}

fn assert_fold_regions(buf: &mut Buffer, expected: &[(usize, usize)]) {
    let regions: Vec<SyntaxFoldRegion> = buf.syntax_fold_regions().to_vec();
    let actual: Vec<(usize, usize)> = regions
        .iter()
        .map(|region| (region.start_line, region.end_line))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn test_markdown_syntax_supports_folding() {
    let buf = markdown_buffer("markdown-fold-support", "# Title\n");

    assert!(buf.syntax_supports_folding());
}

#[test]
fn test_markdown_heading_fold_regions() {
    let mut buf = markdown_buffer(
        "markdown-fold-headings",
        "# A\nintro\n## B\nbody\n## C\nbody\n# D\n",
    );

    assert_fold_regions(&mut buf, &[(2, 3), (4, 5), (0, 5)]);
}

#[test]
fn test_markdown_nested_heading_fold_regions() {
    let mut buf = markdown_buffer(
        "markdown-fold-nested-headings",
        "# A\nintro\n## B\ntext\n### C\ndeep\n# D\nend\n",
    );

    assert_fold_regions(&mut buf, &[(4, 5), (2, 5), (0, 5), (6, 7)]);
}

#[test]
fn test_markdown_heading_without_body_does_not_fold() {
    let mut buf = markdown_buffer("markdown-fold-empty-heading", "# A\n## B\n");

    assert_fold_regions(&mut buf, &[(0, 1)]);
}

#[test]
fn test_markdown_headings_inside_code_fence_do_not_fold() {
    let mut buf = markdown_buffer(
        "markdown-fold-code-fence-heading",
        "# A\nintro\n```markdown\n# Not a heading\n```\n# B\nbody\n",
    );

    assert_fold_regions(&mut buf, &[(0, 4), (5, 6)]);
}

#[test]
fn test_markdown_heading_folds_survive_body_text_edit() {
    let mut buf = markdown_buffer(
        "markdown-fold-body-text-edit",
        "# A\nintro\n## B\nbody\n# C\nend\n",
    );
    assert_fold_regions(&mut buf, &[(2, 3), (0, 3), (4, 5)]);

    buf.insert_text(Cursor::new(1, 0), "more ");

    assert_fold_regions(&mut buf, &[(2, 3), (0, 3), (4, 5)]);
}

#[test]
fn test_markdown_heading_folds_survive_body_line_insert() {
    let mut buf = markdown_buffer(
        "markdown-fold-body-line-insert",
        "# A\nintro\n## B\nbody\n# C\nend\n",
    );
    assert_fold_regions(&mut buf, &[(2, 3), (0, 3), (4, 5)]);

    buf.insert_text(Cursor::new(1, 0), "inserted\n");

    assert_fold_regions(&mut buf, &[(3, 4), (0, 4), (5, 6)]);
}

#[test]
fn test_markdown_code_fence_resolves_canonical_capture() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-fence-js", "md").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path(
        "```javascript\nconst value = null; const count = 1;\n```",
        path,
    );

    let body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    assert!(body.iter().any(|span| span.style == tag("keyword")));
    assert!(body.iter().any(|span| span.style == tag("constant")));
    assert!(body.iter().any(|span| span.style == tag("variable")));
    assert!(body.iter().any(|span| span.style == tag("number")));
}

#[test]
fn test_markdown_code_fence_resolves_alias_capture() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-fence-js-alias", "md").as_path())
        .unwrap();
    let mut buf =
        Buffer::from_str_with_path("```js\nconst value = null; const count = 1;\n```", path);

    let body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    assert!(body.iter().any(|span| span.style == tag("keyword")));
    assert!(body.iter().any(|span| span.style == tag("constant")));
    assert!(body.iter().any(|span| span.style == tag("variable")));
    assert!(body.iter().any(|span| span.style == tag("number")));
}

#[test]
fn test_markdown_fixture_js_closing_fence_uses_code_block_tag() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-fixture", "md").as_path()).unwrap();
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/markdown.md");
    let mut buf = Buffer::from_str_with_path(fixture, path);

    let closing = buf
        .syntax_spans_for_line(22)
        .expect("fixture closing fence should exist");
    assert_eq!(closing.len(), 1);
    assert_eq!(closing[0].style, tag("markup.code"));
}

#[test]
fn test_markdown_fixture_rust_fence_injects_rust_highlighting() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-fixture-rust", "md").as_path()).unwrap();
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/markdown.md");
    let mut buf = Buffer::from_str_with_path(fixture, path);

    let main_line = buf
        .syntax_spans_for_line(14)
        .expect("rust main line should exist");
    let println_line = buf
        .syntax_spans_for_line(16)
        .expect("rust println line should exist");

    assert_spans_include_style(&main_line, tag("keyword"));
    assert_spans_include_style(&main_line, tag("punctuation"));
    assert_spans_include_style(&println_line, tag("function.macro"));
    assert_spans_include_style(&println_line, tag("variable"));
    assert_spans_include_style(&println_line, tag("string"));
    assert_spans_include_style(&println_line, tag("punctuation"));
}

#[test]
fn test_markdown_fixture_highlights_extended_structures() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-markdown-extended", "md").as_path())
            .unwrap();
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/markdown.md");
    let mut buf = Buffer::from_str_with_path(fixture, path);

    let setext = buf
        .syntax_spans_for_line(31)
        .expect("setext underline line should exist");
    let reference_link = buf
        .syntax_spans_for_line(33)
        .expect("reference link line should exist");
    let reference_definition = buf
        .syntax_spans_for_line(34)
        .expect("reference definition line should exist");
    let autolink = buf
        .syntax_spans_for_line(37)
        .expect("autolink line should exist");
    let underscore_line = buf
        .syntax_spans_for_line(39)
        .expect("underscore emphasis line should exist");
    let indented_code = buf
        .syntax_spans_for_line(41)
        .expect("indented code line should exist");
    let tilde_fence = buf
        .syntax_spans_for_line(43)
        .expect("tilde fence line should exist");
    let tilde_body = buf
        .syntax_spans_for_line(44)
        .expect("tilde fence body line should exist");

    assert_spans_include_style(&setext, tag("markup.heading"));
    assert_spans_include_style(&reference_link, tag("markup.link"));
    assert_spans_include_style(&reference_definition, tag("markup.link"));
    assert_spans_include_style(&autolink, tag("markup.link"));
    assert_spans_include_style(&underscore_line, tag("markup.emphasis"));
    assert_spans_include_style(&underscore_line, tag("markup.strong"));
    assert_spans_include_style(&indented_code, tag("markup.code"));
    assert_spans_include_style(&tilde_fence, tag("markup.code"));
    assert_spans_include_style(&tilde_body, tag("function"));
}

#[test]
fn test_markdown_prose_does_not_use_generic_identifier_heuristics() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-prose", "md").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("Capitalized SCREAMY_CASE words stay plain", path);

    let spans = buf
        .syntax_spans_for_line(0)
        .expect("prose line should exist");
    assert!(spans.is_empty());
}

#[test]
fn test_markdown_fixture_highlights_common_constructs() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-markdown-common", "md").as_path())
            .unwrap();
    let fixture = include_str!("../../../../../urvim_syntax/fixtures/markdown.md");
    let mut buf = Buffer::from_str_with_path(fixture, path);

    let heading = buf
        .syntax_spans_for_line(0)
        .expect("heading line should exist");
    let prose = buf
        .syntax_spans_for_line(2)
        .expect("prose line should exist");
    let quote = buf
        .syntax_spans_for_line(6)
        .expect("quote line should exist");
    let list = buf
        .syntax_spans_for_line(8)
        .expect("list line should exist");
    let thematic_break = buf
        .syntax_spans_for_line(11)
        .expect("thematic break line should exist");
    let plain = buf
        .syntax_spans_for_line(28)
        .expect("plain line should exist");

    assert_spans_include_style(&heading, tag("markup.heading"));
    assert_spans_include_style(&prose, tag("markup.emphasis"));
    assert_spans_include_style(&prose, tag("markup.strong"));
    assert_spans_include_style(&prose, tag("markup.code.inline"));
    assert_spans_include_style(&prose, tag("markup.link"));
    assert!(
        prose
            .iter()
            .any(|span| span.style == tag("markup.emphasis.text"))
    );
    assert!(
        prose
            .iter()
            .any(|span| span.style == tag("markup.strong.text"))
    );
    assert!(
        prose
            .iter()
            .any(|span| span.style == tag("markup.code.inline.text"))
    );
    assert_spans_include_style(&quote, tag("markup.quote"));
    assert_spans_include_style(&list, tag("markup.list"));
    assert_spans_include_style(&thematic_break, tag("markup.thematic_break"));
    assert!(plain.is_empty());
}

#[test]
fn test_markdown_code_fence_unknown_capture_is_unstyled() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-fence-unknown", "md").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("```wat\nconst value = 1;\n```", path);

    let body = buf
        .syntax_spans_for_line(1)
        .expect("body line should exist");
    assert!(body.is_empty());
    let closing = buf
        .syntax_spans_for_line(2)
        .expect("closing line should exist");
    assert!(closing.iter().any(|span| span.style == tag("markup.code")));
}
