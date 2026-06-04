use super::*;

#[test]
fn test_dockerfile_fixture_uses_grammar_rules() {
    let fixture = include_str!("fixtures/Dockerfile");
    let mut buf = named_fixture_buffer("dockerfile", fixture);

    let comment = buf
        .syntax_spans_for_line(0)
        .expect("comment line should exist");
    let from_line = buf
        .syntax_spans_for_line(4)
        .expect("from line should exist");
    let env_line = buf
        .syntax_spans_for_line(13)
        .expect("variable line should exist");
    let run_line = buf
        .syntax_spans_for_line(20)
        .expect("run line should exist");

    assert_spans_include_comment_style(&comment);
    assert_spans_include_style(&from_line, tag("keyword"));
    assert_spans_include_style(&env_line, tag("variable"));
    assert_spans_include_style(&env_line, tag("keyword"));
    assert_spans_include_style(&run_line, tag("string"));
}

#[test]
fn test_dockerfile_interpolation_forms_use_variable_style() {
    let mut buf = named_fixture_buffer(
        "dockerfile",
        "FROM ${BASE_IMAGE:-alpine}\nENV MODE=${DEBUG:+debug}\nWORKDIR ${APP_HOME}\nRUN echo \"poetry==$POETRY_VERSION\"",
    );

    let from_line = buf.line_at(0).expect("from line text").to_text();
    let from_spans = buf.syntax_spans_for_line(0).expect("from line spans");
    let env_line = buf.line_at(1).expect("env line text").to_text();
    let env_spans = buf.syntax_spans_for_line(1).expect("env line spans");
    let workdir_line = buf.line_at(2).expect("workdir line text").to_text();
    let workdir_spans = buf.syntax_spans_for_line(2).expect("workdir line spans");
    let run_line = buf.line_at(3).expect("run line text").to_text();
    let run_spans = buf.syntax_spans_for_line(3).expect("run line spans");

    assert_spans_include_exact_style(
        &from_spans,
        &from_line,
        "${BASE_IMAGE:-alpine}",
        tag("variable"),
    );
    assert_spans_include_exact_style(&env_spans, &env_line, "${DEBUG:+debug}", tag("variable"));
    assert_spans_include_exact_style(
        &workdir_spans,
        &workdir_line,
        "${APP_HOME}",
        tag("variable"),
    );
    assert_spans_include_exact_style(&run_spans, &run_line, "$POETRY_VERSION", tag("variable"));
}

#[test]
fn test_dockerfile_escaped_and_incomplete_variables_are_not_variable_style() {
    let mut buf = named_fixture_buffer("dockerfile", "RUN echo \"\\$APP_HOME ${APP_HOME\"");
    let line = buf.line_at(0).expect("line text").to_text();
    let spans = buf.syntax_spans_for_line(0).expect("line spans");

    assert!(!spans.iter().any(|span| {
        span.style == tag("variable") && &line[span.start_byte..span.end_byte] == "$APP_HOME"
    }));
    assert!(!spans.iter().any(|span| {
        span.style == tag("variable") && &line[span.start_byte..span.end_byte] == "${APP_HOME"
    }));
}
