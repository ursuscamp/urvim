# 202604032235: Multi-language syntax highlighting regressions in string and interpolation regions

## Summary
Several built-in syntax fixtures expose visible highlighting regressions in the string
and interpolation paths for Python, Bash, C, C++, Perl, and Rust. The affected
fixtures all render in a way that makes string bodies, interpolation delimiters, or
terminator tokens look inconsistent with the surrounding grammar rules.

## Severity: Medium

The issue is user-visible in core editor functionality, but it does not affect buffer
contents, cursor movement, or file persistence.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Fixtures:
  - `src/buffer/tests/syntax/fixtures/python.py`
  - `src/buffer/tests/syntax/fixtures/bash.sh`
  - `src/buffer/tests/syntax/fixtures/c.c`
  - `src/buffer/tests/syntax/fixtures/cpp.cpp`
  - `src/buffer/tests/syntax/fixtures/perl.pl`
  - `src/buffer/tests/syntax/fixtures/rust.rs`
- Relevant built-in grammars:
  - `src/syntax/builtins/python.toml`
  - `src/syntax/builtins/bash.toml`
  - `src/syntax/builtins/c.toml`
  - `src/syntax/builtins/cpp.toml`
  - `src/syntax/builtins/perl.toml`
  - `src/syntax/builtins/rust.toml`

## Reproduction Steps

1. Open `src/buffer/tests/syntax/fixtures/python.py` with Python syntax highlighting enabled.
2. Inspect the multiline f-string at lines 25-27.
3. Notice that the closing `}` on the interpolation line is styled like string text instead of
   cleanly ending the interpolation region.
4. Open `src/buffer/tests/syntax/fixtures/bash.sh` with Bash syntax highlighting enabled.
5. Inspect the `${MODE}` expansion at line 10 and the `"${targets[0]}"` expansion at line 11.
6. Notice that the expansion is highlighted as string text except for the leading `$`.
7. Open `src/buffer/tests/syntax/fixtures/c.c` and `src/buffer/tests/syntax/fixtures/cpp.cpp`.
8. Inspect the string literals on the printf/fprintf lines.
9. Notice that the body text inside the strings does not remain consistently styled as string text.
10. Open `src/buffer/tests/syntax/fixtures/perl.pl`.
11. Inspect the heredoc terminator at lines 7-9.
12. Notice that the `;` after the EOF marker is highlighted like part of the string instead of the
    heredoc terminator path.
13. Open `src/buffer/tests/syntax/fixtures/rust.rs`.
14. Inspect the formatting strings on lines 24-26.
15. Notice that text inside the format strings, especially content beginning with a capital letter
    such as `Hello`, is not highlighted as ordinary string text.

## Expected Behavior

- Python f-string interpolations should keep the closing `}` outside the string body.
- Bash parameter expansions such as `${MODE}` should highlight the `$`, braces, and variable name
  according to their grammar roles instead of collapsing into string text.
- C and C++ string literals should render their entire body text as string content except for
  explicit escape or interpolation regions.
- Perl heredoc terminators should highlight the exact EOF marker and trailing terminator punctuation
  with the heredoc rule, not with string body styling.
- Rust format strings should keep ordinary text inside the format literal styled as string text,
  including words that begin with a capital letter.

## Actual Behavior

- Python multiline f-strings mis-style the interpolation close brace as string text.
- Bash `${...}` regions render almost entirely as string text, with only the `$` visually distinct.
- C and C++ string bodies lose consistent string-text styling inside ordinary literals.
- Perl heredoc termination styling leaks onto the trailing `;` after the EOF marker.
- Rust format strings apply an incorrect fallback style to capitalized text inside the format
  literal, making ordinary string content look like a language token.

## Impact

- Multiline Python strings become hard to read because interpolation regions do not close cleanly.
- Bash expansions look like plain quoted text instead of shell syntax.
- C and C++ strings become visually noisy and less trustworthy for scanning literal contents.
- Perl heredocs look malformed at the terminator line.
- Rust format strings lose the distinction between literal text and token-like content.

## Root Cause

The regressions appear to come from several grammar-level issues rather than a single renderer bug:

1. Python, Bash, and Perl all rely on region-style string handling, and their opener/closer or
   payload-matching rules are not consistently reserving the closing delimiter for the correct
   context.
2. The C and C++ grammars do not consistently keep the full non-escape body of a string literal
   inside a string span, so ordinary text falls through to broader token heuristics.
3. Rust format strings still appear to use an identifier fallback inside string bodies, which causes
   capitalized words in the format literal to resolve as something other than plain string text.

These symptoms line up with the built-in syntax rules in the affected `src/syntax/builtins/*.toml`
files rather than with buffer editing or rendering state.

## Solution Approach

- Update the affected built-in grammars so each string family has explicit, stable body and
  terminator rules.
- Add or refine interpolation regions for Python, Bash, and Rust so closing delimiters and embedded
  content are classified by the correct context.
- Tighten the C and C++ string-body rules so ordinary literal text always stays in the string span.
- Fix the Perl heredoc terminator rule so the EOF line is handled as a dedicated terminator rather
  than as string content.
- Add focused syntax regression tests for each fixture case to prevent the same styling drift from
  returning.

The chosen fix should stay in the syntax-definition layer. A broader renderer change was considered
but would be the wrong scope because the failures are language-specific and already visible in the
individual builtin grammars.

## Code Changes

- `src/syntax/builtins/python.toml`
  - correct multiline f-string interpolation closing behavior
- `src/syntax/builtins/bash.toml`
  - fix `${...}` expansion styling so the whole expansion is not rendered as string text
- `src/syntax/builtins/c.toml`
  - keep ordinary string bodies consistently styled as string content
- `src/syntax/builtins/cpp.toml`
  - keep ordinary string bodies consistently styled as string content
- `src/syntax/builtins/perl.toml`
  - fix heredoc terminator styling at the EOF line
- `src/syntax/builtins/rust.toml`
  - prevent capitalized text inside format strings from falling through to the wrong fallback style
- `src/buffer/tests/syntax/python.rs`
  - add or adjust regression coverage for multiline f-string closing behavior
- `src/buffer/tests/syntax/bash.rs`
  - add regression coverage for `${...}` styling
- `src/buffer/tests/syntax/c.rs`
  - add regression coverage for ordinary string-body styling
- `src/buffer/tests/syntax/cpp.rs`
  - add regression coverage for ordinary string-body styling
- `src/buffer/tests/syntax/perl.rs`
  - add regression coverage for heredoc terminator styling
- `src/buffer/tests/syntax/rust.rs`
  - add regression coverage for capitalized text inside format strings
- `src/buffer/tests/syntax/fixtures/python.py`
- `src/buffer/tests/syntax/fixtures/bash.sh`
- `src/buffer/tests/syntax/fixtures/c.c`
- `src/buffer/tests/syntax/fixtures/cpp.cpp`
- `src/buffer/tests/syntax/fixtures/perl.pl`
- `src/buffer/tests/syntax/fixtures/rust.rs`
  - extend the fixtures only where additional cases are needed for the regressions

## Edge Cases

- Python raw f-strings and triple-quoted strings should keep multiline state stable.
- Bash command substitutions and arithmetic substitutions should not inherit `${...}` variable
  styling.
- C and C++ format-string handling should not regress while ordinary string text is fixed.
- Perl heredocs with quoted delimiters should continue to match only the declared terminator.
- Rust escaped braces such as `{{` and `}}` should still highlight as string escapes, and ordinary
  non-format strings should remain on the plain string path.
