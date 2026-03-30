# Syntax Correctness Holes 1 - Implementation Tasks

## Overview
Tighten the built-in syntax grammars for Rust, Python, JavaScript, JSON, TOML, Markdown, and shell so the editor highlights the most common lexical forms correctly and has regression coverage for the corrected behavior.

## Backend
- [x] **1.** Correct the Rust built-in grammar and fixture coverage.
  - [x] **1.1** Expand `src/syntax/builtins/rust.toml` to cover the missing lexical families called out in the spec, including richer numeric literals, raw and byte-oriented literal forms, lifetimes, labels, attributes, and doc-comment forms.
  - [x] **1.2** Extend `fixtures/syntax/rust.rs` with examples that exercise the corrected token families and multiline literal behavior.
  - [x] **1.3** Update buffer tests to assert the fixture highlights the intended Rust categories without regressing the existing format-string coverage.
- [x] **2.** Correct the Python built-in grammar and fixture coverage.
  - [x] **2.1** Expand `src/syntax/builtins/python.toml` to cover prefixed strings, raw strings, bytes literals, richer numeric literals, decorators, and formatted-string regions.
  - [x] **2.2** Extend `fixtures/syntax/python.py` with examples that exercise the corrected token families and multiline string behavior.
  - [x] **2.3** Update buffer tests to assert the Python fixture distinguishes the corrected lexical forms.
- [x] **3.** Correct the JavaScript built-in grammar and fixture coverage.
  - [x] **3.1** Expand `src/syntax/builtins/javascript.toml` to cover regex literals, richer numeric literals, private identifiers, class-field-like tokens, and template literal interpolation.
  - [x] **3.2** Extend `fixtures/syntax/javascript.js` with examples that exercise the corrected token families and multiline template behavior.
  - [x] **3.3** Update buffer tests to assert the JavaScript fixture distinguishes the corrected lexical forms.
- [x] **4.** Correct the JSON built-in grammar and fixture coverage.
  - [x] **4.1** Remove the invalid generic identifier heuristic from `src/syntax/builtins/json.toml` and tighten the grammar to valid JSON lexical forms only.
  - [x] **4.2** Extend `fixtures/syntax/json.json` with valid and invalid number, string, boolean, null, and punctuation examples.
  - [x] **4.3** Update buffer tests to assert invalid identifier-like text is not styled as valid JSON structure.
- [x] **5.** Correct the TOML built-in grammar and fixture coverage.
  - [x] **5.1** Expand `src/syntax/builtins/toml.toml` to cover the full TOML number family and clearer key/table structure tagging.
  - [x] **5.2** Extend `fixtures/syntax/toml.toml` with dotted keys, tables, arrays of tables, inline tables, and number forms that exercise the corrected behavior.
  - [x] **5.3** Update buffer tests to assert the TOML fixture highlights keys, tables, and values distinctly.
- [x] **6.** Correct the Markdown built-in grammar and fixture coverage.
  - [x] **6.1** Expand `src/syntax/builtins/markdown.toml` to cover headings, fenced and indented code blocks, lists, blockquotes, links, images, and emphasis/strong forms used in ordinary Markdown.
  - [x] **6.2** Extend `fixtures/syntax/markdown.md` with block and inline examples that exercise the corrected Markdown constructs.
  - [x] **6.3** Update buffer tests to assert Markdown block structure remains stable across multiple lines.
- [x] **7.** Correct the shell built-in grammar and fixture coverage.
  - [x] **7.1** Expand `src/syntax/builtins/shell.toml` to cover comments, expansions, command substitution, arithmetic substitution, heredoc-style regions, and quoted multiline forms.
  - [x] **7.2** Extend `fixtures/syntax/shell.sh` with examples that exercise the corrected token families and multiline shell behavior.
  - [x] **7.3** Update buffer tests to assert shell quoted text and heredoc-style regions remain coherent across line boundaries.
- [x] **8.** Run validation for the updated syntax bundle.
  - [x] **8.1** Run the relevant syntax regression tests for the affected fixtures.
  - [x] **8.2** Run `cargo check` and fix any build errors or warnings introduced by the grammar updates.

## Testing
- [x] **9.** Confirm the first correctness-hole bundle is covered by fixture-driven regression tests.
  - [x] **9.1** Verify each of the seven target grammars has at least one fixture case that would have failed before the grammar corrections.
  - [x] **9.2** Verify the test suite still passes for the existing syntax highlighting coverage not touched by this spec.

## Completion Summary

| Area | Tasks | Status |
| --- | --- | --- |
| Backend | 8 | Complete |
| Testing | 1 | Complete |
| Total | 9 | Complete |
