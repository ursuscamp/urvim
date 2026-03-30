# 202604032258: Multi-language syntax highlighting regressions in punctuation, comments, and interpolation

## Summary
Several built-in syntax fixtures show visible highlighting regressions across shell, CSS, Java, Markdown, TypeScript, and Bash. The affected cases all involve token boundary handling: punctuation that is styled like the surrounding text, comment interiors that do not inherit comment styling, interpolation regions that stay in string styling, or injected code that falls back to the wrong color.

## Severity: Medium

The regressions are user-visible in a core editor feature, but they do not affect buffer contents, cursor movement, or file persistence.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Fixtures:
  - `src/buffer/tests/syntax/fixtures/shell.sh`
  - `src/buffer/tests/syntax/fixtures/css.css`
  - `src/buffer/tests/syntax/fixtures/java.java`
  - `src/buffer/tests/syntax/fixtures/markdown.md`
  - `src/buffer/tests/syntax/fixtures/typescript.ts`
  - `src/buffer/tests/syntax/fixtures/bash.sh`
- Relevant built-in grammars:
  - `src/syntax/builtins/shell.toml`
  - `src/syntax/builtins/css.toml`
  - `src/syntax/builtins/java.toml`
  - `src/syntax/builtins/markdown.toml`
  - `src/syntax/builtins/typescript.toml`
  - `src/syntax/builtins/bash.toml`

## Reproduction Steps

1. Open `src/buffer/tests/syntax/fixtures/shell.sh` with Shell syntax highlighting enabled.
2. Inspect the arithmetic substitution line `echo "$((1 + 2))"`.
3. Notice that the double parentheses around the arithmetic expression are highlighted inconsistently.
4. Open `src/buffer/tests/syntax/fixtures/css.css` with CSS syntax highlighting enabled.
5. Inspect the opening `{` for `body, .card {`.
6. Notice that the brace is styled like part of the selector instead of punctuation.
7. Open `src/buffer/tests/syntax/fixtures/java.java` with Java syntax highlighting enabled.
8. Inspect the `/** doc comment */` line.
9. Notice that the interior text of the documentation comment is not highlighted as comment content.
10. Open `src/buffer/tests/syntax/fixtures/markdown.md` with Markdown syntax highlighting enabled.
11. Inspect the fenced Rust block.
12. Notice that the code inside the `fn main()` block is rendered with the wrong fallback color instead of normal Rust token styles.
13. Open `src/buffer/tests/syntax/fixtures/typescript.ts` with TypeScript syntax highlighting enabled.
14. Inspect the template string `const message = `hello ${value}`;`.
15. Notice that the text inside the `${...}` interpolation is highlighted like string content instead of code.
16. Open `src/buffer/tests/syntax/fixtures/bash.sh` with Bash syntax highlighting enabled.
17. Inspect the `printf '%s\n' "${targets[0]}"` line.
18. Notice that the formatting characters in the printf format string are not highlighted distinctly.

## Expected Behavior

- Shell arithmetic substitutions should highlight both delimiter parens and the arithmetic body consistently.
- CSS rule blocks should highlight `{` and `}` as punctuation.
- Java documentation comments should keep their interior text styled as comments.
- Markdown fenced Rust blocks should render the nested Rust code with the Rust grammar, not the fallback color.
- TypeScript template string interpolations should switch out of string styling while the expression is active.
- Bash format strings should highlight printf formatting characters instead of treating the whole string as undifferentiated text.

## Actual Behavior

- Shell arithmetic substitution delimiters render inconsistently.
- CSS consumes the opening `{` as if it were part of the selector span.
- Java doc comment interiors do not inherit comment styling.
- Markdown fenced Rust code falls through to the wrong highlight color inside the block body.
- TypeScript template expression contents stay string-colored instead of code-colored.
- Bash printf format strings do not distinguish formatting characters from plain string text.

## Impact

- Syntax highlighting looks broken or incomplete in several common language fixtures.
- Delimiter bugs make code blocks and declarations harder to scan quickly.
- The Markdown, TypeScript, and Bash regressions are especially noticeable in everyday editing because they affect code the user expects to read as structured text.

## Root Cause

These issues appear to come from a set of language-specific token boundary regressions rather than a renderer-wide problem. In each case, the grammar either:

1. assigns the wrong tag to a delimiter or interior span,
2. fails to switch context cleanly when entering or leaving an embedded region, or
3. falls back to a generic string-style path where a more specific token class should apply.

The CSS and Java issues look like span classification problems, the Markdown and TypeScript issues look like region/injection boundary problems, and the Bash formatting-string issue looks like a missing or overly broad string rule for printf-style format markers.

## Solution Approach

- Fix each affected syntax definition so delimiter, comment, and interpolation boundaries are tagged with the intended token classes.
- Add regression coverage for each fixture so the same styling drift does not return.
- Keep the fix in the syntax-definition and syntax-test layer rather than changing the renderer, since the failures are grammar-specific.

## Code Changes

- `src/syntax/builtins/shell.toml`
  - correct arithmetic substitution delimiter styling
- `src/syntax/builtins/css.toml`
  - ensure rule-block braces are styled as punctuation
- `src/syntax/builtins/java.toml`
  - keep documentation comment interiors styled as comments
- `src/syntax/builtins/markdown.toml`
  - keep injected Rust fences on the Rust grammar instead of the fallback style
- `src/syntax/builtins/typescript.toml`
  - fix template string interpolation boundary styling
- `src/syntax/builtins/bash.toml`
  - add or correct formatting-string token handling for `printf`-style strings
- `src/buffer/tests/syntax/fixtures/shell.sh`
  - keep the arithmetic substitution case in the regression fixture
- `src/buffer/tests/syntax/fixtures/css.css`
  - keep the brace styling case in the regression fixture
- `src/buffer/tests/syntax/fixtures/java.java`
  - keep the doc comment regression case in the fixture
- `src/buffer/tests/syntax/fixtures/markdown.md`
  - keep the fenced Rust block regression case in the fixture
- `src/buffer/tests/syntax/fixtures/typescript.ts`
  - keep the template interpolation regression case in the fixture
- `src/buffer/tests/syntax/fixtures/bash.sh`
  - keep the printf-format-string regression case in the fixture
- `src/buffer/tests/syntax/*.rs`
  - add or adjust syntax regression tests for each affected language

## Edge Cases

- Shell command substitutions and quoted arithmetic expressions should remain stable.
- CSS at-rules and nested rule blocks should continue to highlight correctly.
- Java block comments and line comments should not regress while doc comments are fixed.
- Markdown prose and non-Rust code fences should keep their current styling.
- TypeScript template strings should still highlight escaped braces and nested expressions correctly.
- Bash normal strings, heredocs, and parameter expansions should continue to behave as they do today.
