# HTML attribute entity highlighting - Implementation Tasks

## Overview
Fix HTML syntax highlighting so escaped entities inside quoted attribute values keep both the surrounding string styling and the entity-level styling, then lock the behavior down with a focused regression test and a build check.

## Backend
- [x] **1.** Update the HTML built-in grammar so entities inside quoted attribute values are recognized within the string body.
  - [x] **1.1** Adjust `src/syntax/builtins/html.toml` so quoted attribute values can still surface `&...;` sequences as entity highlighting instead of treating the full attribute text as plain string content.
  - [x] **1.2** Keep standalone entity handling outside attribute values unchanged so ordinary HTML text still highlights correctly.

## Testing
- [x] **2.** Add or tighten regression coverage for entity highlighting inside HTML attribute strings. (depends on: 1)
  - [x] **2.1** Update `src/buffer/tests/syntax/html.rs` to assert that the `alt="Hi &amp; bye"` attribute value includes both string styling and entity styling.
  - [x] **2.2** Update `src/buffer/tests/syntax/fixtures/html.html` only if a smaller or clearer attribute-entity example is needed for the regression test.

- [x] **3.** Validate the fix with focused tests and a build check. (depends on: 1, 2)
  - [x] **3.1** Run the HTML syntax regression test covering the attribute-string entity case.
  - [x] **3.2** Run `cargo check` to confirm the grammar change builds cleanly.
  - [x] **3.3** Re-run the HTML syntax test if `cargo check` or the grammar edit requires follow-up adjustments.

## Completion Summary

| Area | Tasks | Completed | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 1 | 1 | 0 |
| Testing | 2 | 2 | 0 |
| Total | 3 | 3 | 0 |
