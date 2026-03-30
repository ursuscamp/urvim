# Rust Format String Syntax Highlighting - Implementation Tasks

## Overview
This work refines Rust syntax highlighting so formatting strings follow the `std::fmt` format-string rules while ordinary Rust strings remain unchanged.

## Backend
- [x] **1.** Update the Rust builtin syntax definition to model `std::fmt` format-string syntax inside formatting-macro call contexts.
  - [x] **1.1** Add or adjust the formatting-macro context so the first string literal argument enters the specialized format-string rule set.
  - [x] **1.2** Add rules for placeholder regions, including implicit, positional, and named forms.
  - [x] **1.3** Add rules for escaped braces `{{` and `}}` so they remain literal string content.
  - [x] **1.4** Preserve the existing plain-string path for non-format strings in Rust files.

- [x] **2.** Review syntax tag usage and documentation for the new format-string styling behavior.
  - [x] **2.1** Confirm the built-in Rust syntax uses the standard tags expected by the theme system.
  - [x] **2.2** Update `docs/syntax/tags.md` only if a missing tag or child tag is required for clearer styling semantics.

## Testing
- [x] **3.** Add regression coverage for Rust format-string highlighting.
  - [x] **3.1** Extend `fixtures/syntax/rust.rs` with examples that cover placeholders, named arguments, format specifiers, and escaped braces.
  - [x] **3.2** Add tests that verify `format!("Hello, {}!", name)` highlights the macro context, format string, and placeholder regions distinctly.
  - [x] **3.3** Add tests that verify `format!("{name:04}")` highlights named placeholders and format-specifier content.
  - [x] **3.4** Add a regression test that proves `let s = "hello {name}";` remains ordinary string highlighting.
  - [x] **3.5** Add an edit-invalidation test to ensure format-string highlighting updates after inserting or removing text near the call.

## Completion Summary
| Area | Total | Done | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 2 | 2 | 0 |
| Testing | 1 | 1 | 0 |
| Total | 3 | 3 | 0 |
