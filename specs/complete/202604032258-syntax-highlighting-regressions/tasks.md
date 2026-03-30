# Multi-language syntax highlighting regressions - Implementation Tasks

## Overview

Fix the visible highlighting regressions in the shell, CSS, Java, Markdown, TypeScript,
and Bash built-in grammars, then lock the behavior down with fixture-driven regression
tests and a final build check.

## Backend

- [x] **1.** Update the shell and Bash built-in grammars so delimiter and formatting-string boundaries render correctly.
  - [x] **1.1** Adjust `src/syntax/builtins/shell.toml` so the `"$((1 + 2))"` arithmetic substitution keeps both closing parens on the intended punctuation path.
  - [x] **1.2** Review `src/syntax/builtins/bash.toml` formatting-string handling so `printf`-style format characters are distinguished from ordinary string text.

- [x] **2.** Update the CSS and Java built-in grammars so braces and doc-comment interiors keep their intended token classes.
  - [x] **2.1** Adjust `src/syntax/builtins/css.toml` so opening and closing rule braces are styled as punctuation rather than selector text.
  - [x] **2.2** Review `src/syntax/builtins/java.toml` doc-comment rules so the interior of `/** ... */` stays on the comment path instead of falling through to plain text.

- [x] **3.** Update the Markdown and TypeScript built-in grammars so injected code and template-string interpolations switch contexts cleanly.
  - [x] **3.1** Review `src/syntax/builtins/markdown.toml` so fenced Rust blocks keep the injected Rust grammar active for the entire fence body.
  - [x] **3.2** Adjust `src/syntax/builtins/typescript.toml` so template string interpolations highlight the embedded expression as code instead of string text.

- [x] **4.** Extend the syntax fixtures with focused regression cases for each affected language. (depends on: 1, 2, 3)
  - [x] **4.1** Keep `src/buffer/tests/syntax/fixtures/shell.sh` aligned with the arithmetic substitution regression case.
  - [x] **4.2** Keep `src/buffer/tests/syntax/fixtures/css.css` aligned with the rule-brace regression case.
  - [x] **4.3** Keep `src/buffer/tests/syntax/fixtures/java.java` aligned with the doc-comment regression case.
  - [x] **4.4** Keep `src/buffer/tests/syntax/fixtures/markdown.md` aligned with the fenced Rust block regression case.
  - [x] **4.5** Keep `src/buffer/tests/syntax/fixtures/typescript.ts` aligned with the template interpolation regression case.
  - [x] **4.6** Keep `src/buffer/tests/syntax/fixtures/bash.sh` aligned with the printf-format-string regression case.

## Testing

- [x] **5.** Add or adjust syntax regression tests for the affected fixtures. (depends on: 4)
  - [x] **5.1** Update the shell syntax tests to assert arithmetic substitution delimiters keep the expected punctuation styling.
  - [x] **5.2** Update the CSS syntax tests to assert rule braces remain punctuation.
  - [x] **5.3** Update the Java syntax tests to assert doc-comment interiors are still styled as comments.
  - [x] **5.4** Update the Markdown syntax tests to assert fenced Rust blocks continue to inject Rust highlighting.
  - [x] **5.5** Update the TypeScript syntax tests to assert interpolation contents are styled as code, not as plain string text.
  - [x] **5.6** Update the Bash syntax tests to assert printf-format characters are highlighted distinctly.

- [x] **6.** Validate the syntax fixes with the project build and targeted test coverage. (depends on: 1, 2, 3, 5)
  - [x] **6.1** Run the focused syntax regression tests for Shell, CSS, Java, Markdown, TypeScript, and Bash.
  - [x] **6.2** Run `cargo check` to verify the build and catch warnings introduced by the grammar updates.
  - [x] **6.3** Re-run the relevant focused tests if `cargo check` or grammar edits require follow-up fixes.

## Completion Summary

| Area | Tasks | Completed | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 4 | 4 | 0 |
| Testing | 2 | 2 | 0 |
| Total | 6 | 6 | 0 |
