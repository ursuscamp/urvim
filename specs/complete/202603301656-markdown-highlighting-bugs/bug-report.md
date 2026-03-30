# 202603301656: Markdown fenced block and prose highlighting regressions

## Summary
Markdown highlighting has two visible regressions in `fixtures/syntax/markdown.md`:
the closing fence of fenced code blocks is not highlighted consistently, and the
shared syntax tokenizer still applies generic heuristics that style ordinary words
based on capitalization instead of relying only on explicit grammar rules.

## Severity: Medium

The bug is user-visible in a core editor feature, but it does not corrupt buffer
contents or block editing.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Fixture: `fixtures/syntax/markdown.md`
- Relevant implementation: `src/buffer/syntax.rs`, `src/window/view.rs`,
  `src/syntax_builtin/markdown.toml`

## Reproduction Steps

1. Open `fixtures/syntax/markdown.md` in urvim with Markdown syntax highlighting enabled.
2. Inspect the fenced Rust block:
   - the opening ``` fence is styled as punctuation
   - the closing ``` fence is not highlighted correctly
3. Inspect the fenced JavaScript block:
   - the opening ``` fence is styled as punctuation
   - the closing ``` fence renders with the wrong style or no style at all
4. Inspect the fenced WAT block:
   - the opening ``` fence is styled as punctuation
   - the closing ``` fence is not highlighted
5. Inspect the non-code prose near the top of the file.
6. Notice that capitalized words and all-caps words are highlighted differently from
   plain lowercase prose, even though the Markdown grammar does not define those
   tokens.

## Expected Behavior

- Both the opening and closing fence delimiters should render with the Markdown fence
  style.
- Syntax highlighting should come only from explicit grammar rules.
- Markdown prose should remain mostly unstyled except for explicit Markdown constructs
  such as headings, inline code, and links.
- Capitalized prose words should not be classified as types, constants, or keywords
  unless the active grammar explicitly defines them.

## Actual Behavior

- The opening fence delimiter is highlighted, but the closing fence delimiter is
  emitted inconsistently and often appears unstyled.
- The shared tokenizer still applies generic identifier heuristics after grammar
  matching, so ordinary prose words can receive code-like styling even when the
  grammar did not declare them.

## Impact

- Fenced code blocks look incomplete or broken because their closing delimiters do not
  match the opening delimiter styling.
- Regular Markdown paragraphs become visually noisy and harder to read.
- The Markdown fixture no longer reflects the expected “prose plus explicit Markdown
  syntax” presentation.

## Root Cause

There are two related causes:

1. In `src/buffer/syntax.rs`, `tokenize_region_body()` emits the closing region span
   with `SyntaxSpan::new(close, close + region.end.len(), region.style)`. The `close`
   value already points to the byte immediately after the delimiter, so the emitted
   span starts past the delimiter instead of covering it. The renderer then clamps or
   skips the out-of-bounds span, leaving the closing fence unstyled.
2. Markdown text is tokenized by the generic code-style path in
   `tokenize_code_line()`. That path treats identifier-like words according to the
   syntax definition, so prose can pick up code-style highlighting when the grammar
   does not explicitly define those tokens.

## Solution Approach

- Fix the closing fence span to cover the delimiter itself rather than the bytes after
  it.
- Remove the shared tokenizer's generic identifier heuristics so syntax highlighting
  comes only from explicit grammar rules.
- Keep explicit grammar-driven tokenization for comments, strings, numbers, regions,
  punctuation, operators, keywords, constants, and types that are declared in the
  syntax definition.

The fence fix is the direct rendering bug. The heuristic removal is the broader
tokenizer change requested in discussion so the same rule applies to every grammar.

## Code Changes

- `src/buffer/syntax.rs`
  - correct the closing delimiter span for injected regions
  - remove the fallback capitalization heuristics from shared tokenization
- `src/buffer/tests.rs`
  - add regression coverage for closing fence styling
  - add regression coverage ensuring prose words are not highlighted by generic
    identifier heuristics
- `fixtures/syntax/markdown.md`
  - keep the fixture aligned with the Markdown regression cases

## Edge Cases

- Unknown or missing fence languages should continue to render unstyled inside the
  fence body.
- Multiline injected regions should still preserve nested syntax state across lines.
- Inline Markdown constructs such as backticks and links should continue to highlight.
- Explicit syntax keywords, constants, and types should still highlight where the
  grammar defines them.
- The fix should not reintroduce broad capitalization-based styling in any filetype.
