# Markdown Syntax Highlighting - Implementation Tasks

## Overview

Expand the built-in Markdown grammar, update shipped theme coverage for the markup tag family, and add regression tests that prove common Markdown constructs highlight correctly while plain prose stays quiet.

## Backend

- [x] **1.** Expand the built-in Markdown grammar in [`src/syntax_builtin/markdown.toml`](/Users/ryan/Dev/urvim/src/syntax_builtin/markdown.toml)
  - [x] **1.1** Replace the current minimal rule set with explicit rules for headings, emphasis, strong text, inline code, links, blockquotes, lists, thematic breaks, and fenced code blocks
  - [x] **1.2** Tag Markdown constructs with the documented `markup.*` vocabulary, including `markup.code` and `markup.code.inline`, plus `.text` refinements where body text should be styleable separately from delimiters or markers
  - [x] **1.3** Preserve fenced-code language capture so known fence languages continue to inject nested syntax
  - [x] **1.4** Ensure unknown fence languages still leave the fence body unstyled while keeping the fence delimiters highlighted

- [x] **2.** Update the standard syntax tag documentation in [`docs/syntax/tags.md`](/Users/ryan/Dev/urvim/docs/syntax/tags.md)
  - [x] **2.1** Add the Markdown code-span and code-block tags to the recommended markup vocabulary
  - [x] **2.2** Keep the `.text` refinement guidance aligned with the Markdown grammar changes
  - [x] **2.3** Ensure the documentation still presents `markup.strong` and `markup.emphasis` as the canonical Markdown emphasis tags

- [x] **3.** Update built-in theme coverage for Markdown markup tags in [`src/theme/builtin/*.toml`](/Users/ryan/Dev/urvim/src/theme/builtin)
  - [x] **3.1** Add a base `markup` syntax style to each built-in theme so Markdown highlighting remains visible by default
  - [x] **3.2** Add targeted overrides for the most useful Markdown refinements, such as headings, emphasis, strong text, code spans, links, lists, blockquotes, and fenced code blocks
  - [x] **3.3** Verify hierarchical fallback still works when a theme defines only `markup` and not every child tag

- [x] **4.** Refresh the Markdown fixture in [`fixtures/syntax/markdown.md`](/Users/ryan/Dev/urvim/fixtures/syntax/markdown.md)
  - [x] **4.1** Add representative examples for the Markdown constructs covered by the new grammar rules
  - [x] **4.2** Keep a plain-prose section in the fixture so the tests can confirm ordinary text is not styled accidentally
  - [x] **4.3** Keep fenced examples for at least one recognized language and one unrecognized language

## Testing

- [x] **5.** Add and update regression tests in [`src/buffer/tests.rs`](/Users/ryan/Dev/urvim/src/buffer/tests.rs)
  - [x] **5.1** Add assertions for headings, emphasis, strong text, inline code spans, links, blockquotes, lists, and thematic breaks in the Markdown fixture
  - [x] **5.2** Add assertions that plain Markdown prose remains unstyled
  - [x] **5.3** Add assertions that recognized fenced code blocks still inject nested syntax
  - [x] **5.4** Add assertions that unknown fence languages leave the body unstyled but keep the fence delimiters highlighted

- [x] **6.** Run validation
  - [x] **6.1** Run the focused syntax and buffer tests that cover Markdown rendering and injected fences
  - [x] **6.2** Run `cargo check` to verify the build and catch warnings

## Completion Summary

| Area | Status | Notes |
| --- | --- | --- |
| Backend | Complete | Markdown grammar, docs, themes, and fixture |
| Testing | Complete | Regression assertions and validation |
| Total | 6/6 complete | 100% |
