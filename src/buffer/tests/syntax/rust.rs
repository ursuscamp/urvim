use super::*;

#[test]
fn test_rust_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-fixture", "rs", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let function_line = buf
        .syntax_spans_for_line(3)
        .expect("function line should exist");
    let function_line_text = buf
        .line_at(3)
        .expect("function line should exist")
        .to_string();
    let type_line = buf
        .syntax_spans_for_line(4)
        .expect("type line should exist");
    let block_line = buf
        .syntax_spans_for_line(14)
        .expect("block line should exist");
    let block_line_text = buf
        .line_at(14)
        .expect("block line should exist")
        .to_string();
    let operator_line = buf
        .syntax_spans_for_line(12)
        .expect("operator line should exist");
    let char_line = buf
        .syntax_spans_for_line(6)
        .expect("char line should exist");
    let escaped_char_line = buf
        .syntax_spans_for_line(7)
        .expect("escaped char line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_exact_style(&function_line, &function_line_text, "{", tag("punctuation"));
    assert_spans_include_style(&type_line, tag("type"));
    assert_spans_include_style(&type_line, tag("punctuation"));
    assert_spans_include_style(&type_line, tag("operator"));
    assert_spans_include_style(&operator_line, tag("operator"));
    assert_spans_include_style(&operator_line, tag("keyword"));
    assert_spans_include_style(&operator_line, tag("punctuation"));
    assert_spans_include_exact_style(&block_line, &block_line_text, "}", tag("punctuation"));
    assert_spans_include_exact_style(&block_line, &block_line_text, "{", tag("punctuation"));
    assert_spans_include_style(&char_line, tag("constant"));
    assert_spans_include_style(&escaped_char_line, tag("constant"));
}

#[test]
fn test_rust_character_literals_use_constant_rules() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-char", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("let a = 'x'; let b = '\\n'; let c = b'\\t';", path);

    let line = buf.line_at(0).expect("line should exist").to_string();
    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    for literal in ["'x'", "'\\n'", "b'\\t'"] {
        let start = line.find(literal).expect("literal should exist");
        let end = start + literal.len();
        assert!(spans.iter().any(|span| {
            span.style == tag("constant") && span.start_byte <= start && span.end_byte >= end
        }));
    }
}

#[test]
fn test_rust_fixture_highlights_extended_literals() {
    let fixture = include_str!("fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-extended", "rs", fixture);

    let doc_comment = buf
        .syntax_spans_for_line(29)
        .expect("doc comment line should exist");
    let attribute = buf
        .syntax_spans_for_line(31)
        .expect("attribute line should exist");
    let attribute_line = buf
        .line_at(31)
        .expect("attribute line should exist")
        .to_string();
    let raw_string = buf
        .syntax_spans_for_line(33)
        .expect("raw string line should exist");
    let raw_multiline = buf
        .syntax_spans_for_line(34)
        .expect("raw multiline line should exist");
    let byte_string = buf
        .syntax_spans_for_line(36)
        .expect("byte string line should exist");
    let raw_bytes = buf
        .syntax_spans_for_line(37)
        .expect("raw byte string line should exist");
    let numeric = buf
        .syntax_spans_for_line(39)
        .expect("numeric line should exist");
    let namespace = buf
        .syntax_spans_for_line(44)
        .expect("namespace line should exist");

    assert_spans_include_style(&doc_comment, tag("comment.documentation"));
    assert_spans_include_exact_style(
        &attribute,
        attribute_line.as_str(),
        "#[",
        tag("punctuation"),
    );
    assert_spans_include_exact_style(&attribute, attribute_line.as_str(), "]", tag("punctuation"));
    assert_spans_include_style(&raw_string, tag("string"));
    assert_spans_include_style(&raw_multiline, tag("string"));
    assert_spans_include_style(&byte_string, tag("string"));
    assert_spans_include_style(&raw_bytes, tag("string"));
    assert_spans_include_style(&numeric, tag("number"));
    assert_spans_include_style(&namespace, tag("namespace"));
    assert_spans_include_style(&namespace, tag("function"));
    assert!(raw_multiline.iter().any(|span| span.style == tag("string")));
}

#[test]
fn test_rust_strings_use_escape_regions() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-string", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("let msg = \"hello\\nworld\";", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("punctuation"));
}

#[test]
fn test_rust_format_macro_highlights_context_sensitive_format_string() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-format", "rs").as_path()).unwrap();
    let mut buf = Buffer::from_str_with_path("let msg = format!(\"hello {name}\");", path);

    let line = buf.line_at(0).expect("line should exist").to_string();
    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function.macro"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("string"));
    assert_spans_include_style(&spans, tag("variable"));
    assert!(spans.iter().any(|span| span.style == tag("string")
        && line[span.start_byte..span.end_byte].contains("hello ")));
}

#[test]
fn test_rust_fixture_format_strings_follow_std_fmt_rules() {
    let fixture = include_str!("fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-fixture-fmt", "rs", fixture);

    let positional = buf
        .syntax_spans_for_line(24)
        .expect("positional format line should exist");
    let specifier = buf
        .syntax_spans_for_line(25)
        .expect("specifier format line should exist");
    let escaped = buf
        .syntax_spans_for_line(26)
        .expect("escaped format line should exist");
    let escaped_line = buf.line_at(26).expect("escaped format line should exist");
    let escaped_body_start = escaped_line.find('"').expect("opening quote should exist") + 1;
    let escaped_body_end = escaped_line.rfind('"').expect("closing quote should exist");
    let escaped_body = escaped
        .iter()
        .filter(|span| span.start_byte >= escaped_body_start && span.end_byte <= escaped_body_end)
        .collect::<Vec<_>>();
    let specifier_line = buf
        .line_at(25)
        .expect("specifier format line should exist")
        .to_string();

    assert_spans_include_style(&positional, tag("function.macro"));
    assert_spans_include_style(&positional, tag("string"));
    assert_spans_include_style(&positional, tag("punctuation"));
    assert_spans_include_style(&positional, tag("variable"));

    assert_spans_include_style(&specifier, tag("function.macro"));
    assert_spans_include_style(&specifier, tag("string"));
    assert_spans_include_style(&specifier, tag("punctuation"));
    assert_spans_include_style(&specifier, tag("variable"));
    assert_spans_include_style(&specifier, tag("number"));
    assert_spans_include_exact_style(
        &specifier,
        specifier_line.as_str(),
        "value",
        tag("variable"),
    );

    assert_spans_include_style(&escaped, tag("function.macro"));
    assert_spans_include_style(&escaped, tag("string"));
    assert_spans_include_style(&escaped, tag("string.escape"));
    assert!(
        !escaped_body
            .iter()
            .any(|span| span.style == tag("variable"))
    );
    assert!(!escaped_body.iter().any(|span| span.style == tag("number")));
}

#[test]
fn test_rust_format_string_keeps_capitalized_text_as_string() {
    let fixture = include_str!("fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-format-string-body", "rs", fixture);

    let spans = buf
        .syntax_spans_for_line(24)
        .expect("format string line should exist");
    let line = buf.line_at(24).expect("format string line should exist");
    let hello_start = line.find("Hello").expect("capitalized text should exist");
    let hello_end = hello_start + "Hello".len();

    assert!(spans.iter().any(|span| {
        span.start_byte <= hello_start && hello_start < span.end_byte && span.style == tag("string")
    }));
    assert!(!spans.iter().any(|span| {
        span.start_byte <= hello_start
            && hello_start < span.end_byte
            && span.style == tag("variable")
    }));
    assert!(spans.iter().any(|span| {
        span.start_byte <= hello_start && span.end_byte >= hello_end && span.style == tag("string")
    }));
}

#[test]
fn test_rust_non_format_string_remains_plain() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-plain-string", "rs").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("\"hello {name}\"", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("string"));
    assert!(!spans.iter().any(|span| span.style == tag("function.macro")));
    assert!(!spans.iter().any(|span| span.style == tag("variable")));
    assert!(!spans.iter().any(|span| span.style == tag("punctuation")));
}

#[test]
fn test_rust_format_macro_highlighting_updates_after_edit() {
    let path =
        AbsolutePath::from_path(temp_path_with_ext("syntax-rust-format-edit", "rs").as_path())
            .unwrap();
    let mut buf = Buffer::from_str_with_path("format!(\"hello {name}\")", path);

    assert_spans_include_style(
        &buf.syntax_spans_for_line(0).expect("line should exist"),
        tag("function.macro"),
    );

    buf.insert_text(Cursor::new(0, 0), "let msg = ");

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function.macro"));
    assert_spans_include_style(&spans, tag("string"));
}

#[test]
fn test_rust_function_call_highlights_function_name() {
    let path = AbsolutePath::from_path(temp_path_with_ext("syntax-rust-function", "rs").as_path())
        .unwrap();
    let mut buf = Buffer::from_str_with_path("let value = compute(answer);", path);

    let spans = buf.syntax_spans_for_line(0).expect("line should exist");
    assert_spans_include_style(&spans, tag("function"));
    assert_spans_include_style(&spans, tag("punctuation"));
    assert_spans_include_style(&spans, tag("variable"));
}

#[test]
fn test_rust_fixture_highlights_global_identifiers() {
    let fixture = include_str!("fixtures/rust.rs");
    let mut buf = fixture_buffer("syntax-rust-global", "rs", fixture);

    let global_line = buf
        .syntax_spans_for_line(48)
        .expect("global variable line should exist");
    let global_line_text = buf
        .line_at(48)
        .expect("global variable line should exist")
        .to_string();
    let global_mut_line = buf
        .syntax_spans_for_line(49)
        .expect("mutable global variable line should exist");
    let global_mut_line_text = buf
        .line_at(49)
        .expect("mutable global variable line should exist")
        .to_string();

    assert_spans_include_style(&global_line, tag("keyword"));
    assert_spans_include_exact_style(
        &global_line,
        global_line_text.as_str(),
        "GLOBAL_VARIABLES",
        tag("variable.global"),
    );
    assert_spans_include_exact_style(
        &global_mut_line,
        global_mut_line_text.as_str(),
        "GLOBAL_STATE",
        tag("variable.global"),
    );
}
