# Markdown Syntax Highlighting

## Summary
urvim should provide a more complete Markdown syntax highlighting experience for `.md` and `.markdown` files. The Markdown grammar should highlight common structural constructs explicitly, keep ordinary prose mostly unstyled, and use the documented markup tag vocabulary from `docs/syntax/tags.md` rather than code-oriented fallback categories.

## Problem Statement
The current Markdown syntax definition only highlights a small subset of Markdown constructs. As a result, Markdown files do not clearly distinguish structural elements such as headings, emphasis, strong text, lists, blockquotes, links, inline code, and fenced code blocks. Some Markdown content also risks being styled with generic non-Markdown categories, which makes plain prose look noisier than it should.

## User Stories
- As a writer, I want common Markdown constructs to be highlighted consistently, so that documents are easier to scan and edit.
- As a grammar author, I want Markdown tokens to use the documented markup tags, so that the syntax stays aligned with the project’s shared tag vocabulary.
- As a theme author, I want headings, emphasis, strong text, links, lists, blockquotes, and code fences to map to predictable markup tags, so that I can style Markdown broadly without special-casing every file.
- As a user, I want regular prose in Markdown files to stay mostly unstyled, so that highlighted syntax stands out instead of competing with body text.
- As a maintainer, I want fenced code blocks to continue supporting injected syntax when a language is recognized, so that Markdown documents can contain highlighted examples.

## Functional Requirements
- [ ] **REQ-001**: Markdown syntax highlighting shall cover the common Markdown constructs used for authoring prose, including headings, emphasis, strong text, inline code, links, blockquotes, lists, thematic breaks, and fenced code blocks.
- [ ] **REQ-002**: Markdown syntax highlighting shall keep ordinary prose unstyled unless the text matches an explicit Markdown construct.
- [ ] **REQ-003**: Markdown emphasis and strong text shall use the documented markup tags for those concepts, rather than generic code-oriented tags.
- [ ] **REQ-004**: Markdown highlighting shall distinguish the structural parts of links and related inline constructs enough for themes to style delimiters, visible text, and destinations separately when the syntax provides those spans.
- [ ] **REQ-005**: Markdown list items, headings, and blockquotes shall support styling the visible body text separately from surrounding punctuation or markers when that improves theme control.
- [ ] **REQ-006**: Markdown fenced code blocks shall support language capture and injected syntax when the fence language resolves to a known syntax or alias.
- [ ] **REQ-007**: Markdown fenced code blocks with an unknown or missing language shall still render the fence delimiters correctly while leaving the body unstyled.
- [ ] **REQ-008**: Markdown syntax rules shall use the documented standard tag vocabulary from `docs/syntax/tags.md`, including existing markup tags such as `markup.heading`, `markup.code`, `markup.code.inline`, `markup.link`, `markup.list`, `markup.quote`, `markup.strong`, and `markup.emphasis`.
- [ ] **REQ-009**: Markdown highlighting changes shall be covered by regression tests using representative Markdown fixtures that exercise both supported constructs and plain prose.
- [ ] **REQ-010**: The Markdown syntax definition shall continue to recognize files with the `.md` and `.markdown` extensions.

## Non-Functional Requirements
- **Usability**: Markdown files should be easier to read and edit because the visible styling should focus on Markdown structure instead of accidental prose classification.
- **Compatibility**: The Markdown grammar should fit the existing syntax/tag and theme resolution model without requiring special-case rendering paths for Markdown.
- **Maintainability**: Markdown highlighting rules should stay aligned with the documented tag vocabulary so future grammar additions remain predictable for theme authors.
- **Reliability**: Invalid Markdown syntax data or unsupported fence languages should fail or fall back deterministically instead of producing inconsistent styling.

## Acceptance Criteria
- [ ] **AC-001**: A Markdown fixture containing headings, emphasis, strong text, inline code, links, lists, blockquotes, and fenced code blocks highlights each of those constructs with explicit Markdown-oriented tags.
- [ ] **AC-002**: Plain prose in a Markdown file remains unstyled except where it matches an explicit Markdown construct.
- [ ] **AC-003**: Markdown emphasis and strong text are rendered using `markup.emphasis` and `markup.strong`.
- [ ] **AC-004**: A recognized fenced code block language still injects the nested syntax for the fence body.
- [ ] **AC-005**: An unrecognized fenced code block language leaves the fence body unstyled while keeping the fence delimiters styled correctly.
- [ ] **AC-006**: The Markdown fixture suite includes at least one regression case that demonstrates the intended styling of representative Markdown prose and markup constructs.

## Out of Scope
- Full CommonMark parser parity.
- Tables, footnotes, and other extended Markdown dialect features unless they are added in a later scoped change.
- HTML rendering or sanitization behavior inside Markdown files.
- Semantic understanding of Markdown links beyond syntax highlighting.

## Assumptions
- `markup.strong` and `markup.emphasis` are the canonical tags for Markdown strong and emphasis styling.
- The current tag vocabulary in `docs/syntax/tags.md` is sufficient for the Markdown constructs in this stage.
- Built-in syntax definitions and built-in themes can be updated together as part of the same change set.
- Markdown highlighting should continue to use the existing syntax/tag resolution path rather than a Markdown-specific renderer.

## Dependencies
- Existing syntax grammar loading and tag validation.
- Existing theme tag resolution.
- `docs/syntax/tags.md` as the shared markup tag reference.
- Markdown regression fixtures and syntax tests.
