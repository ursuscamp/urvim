# Multi-language syntax highlighting regressions - Implementation Tasks

## Overview
Fix the visible string, interpolation, and terminator highlighting regressions in the
Python, Bash, C, C++, Perl, and Rust built-in grammars, then lock the behavior down
with fixture-driven regression tests and a final build check.

## Backend
- [x] **1.** Update the Python and Bash built-in grammars so interpolation and string-body boundaries render correctly.
  - [x] **1.1** Adjust `src/syntax/builtins/python.toml` so multiline f-strings keep the closing `}` on the interpolation path instead of styling it like string text.
  - [x] **1.2** Review the Python string and raw-string context rules around triple-quoted forms to ensure multiline f-strings and raw multiline strings keep stable state across lines.
  - [x] **1.3** Adjust `src/syntax/builtins/bash.toml` so `${...}` expansions inside double-quoted strings keep the variable, braces, and surrounding string text on the intended shell-syntax paths.

- [x] **2.** Update the C and C++ built-in grammars so ordinary string bodies stay consistently styled as string content.
  - [x] **2.1** Review `src/syntax/builtins/c.toml` string-body rules so ordinary string text does not fall through to broader token heuristics.
  - [x] **2.2** Review `src/syntax/builtins/cpp.toml` string-body rules so ordinary string text stays inside the string span outside of explicit escape or interpolation regions.

- [x] **3.** Update the Perl and Rust built-in grammars for terminator and format-string correctness.
  - [x] **3.1** Adjust `src/syntax/builtins/perl.toml` so heredoc EOF termination highlights the terminator path, not the trailing punctuation as string text.
  - [x] **3.2** Adjust `src/syntax/builtins/rust.toml` so ordinary text inside format strings stays on the string path instead of falling back to identifier-like styling.

- [x] **4.** Extend the syntax fixtures with focused regression cases for each affected language. (depends on: 1, 2, 3)
  - [x] **4.1** Update `src/buffer/tests/syntax/fixtures/python.py` with a minimal case that keeps the multiline f-string interpolation boundary visible.
  - [x] **4.2** Update `src/buffer/tests/syntax/fixtures/bash.sh` with a minimal `${...}` expansion case that exposes the string/variable split.
  - [x] **4.3** Update `src/buffer/tests/syntax/fixtures/c.c` and `src/buffer/tests/syntax/fixtures/cpp.cpp` only if needed to isolate the ordinary string-body regression.
  - [x] **4.4** Update `src/buffer/tests/syntax/fixtures/perl.pl` with a heredoc case that makes the EOF terminator styling explicit.
  - [x] **4.5** Update `src/buffer/tests/syntax/fixtures/rust.rs` with a format-string case that includes capitalized literal text and preserves plain string styling.

## Testing
- [x] **5.** Add or adjust syntax regression tests for the affected fixtures. (depends on: 4)
  - [x] **5.1** Update `src/buffer/tests/syntax/python.rs` to assert the multiline f-string interpolation boundary is classified correctly.
  - [x] **5.2** Update `src/buffer/tests/syntax/bash.rs` to assert `${...}` expansions do not collapse into plain string styling.
  - [x] **5.3** Update `src/buffer/tests/syntax/c.rs` and `src/buffer/tests/syntax/cpp.rs` to assert ordinary string bodies remain styled as string content.
  - [x] **5.4** Update `src/buffer/tests/syntax/perl.rs` to assert the heredoc terminator path covers the EOF marker and trailing punctuation correctly.
  - [x] **5.5** Update `src/buffer/tests/syntax/rust.rs` to assert capitalized text inside format strings stays on the string path.

- [x] **6.** Validate the syntax fixes with the project build and targeted test coverage. (depends on: 1, 2, 3, 5)
  - [x] **6.1** Run the focused syntax regression tests for Python, Bash, C, C++, Perl, and Rust.
  - [x] **6.2** Run `cargo check` and fix any compile errors or warnings introduced by the grammar updates.
  - [x] **6.3** Re-run the relevant focused tests if `cargo check` or grammar edits require follow-up fixes.

## Completion Summary

| Area | Tasks | Completed | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 4 | 4 | 0 |
| Testing | 2 | 2 | 0 |
| Total | 6 | 6 | 0 |
