# Markdown Syntax Highlighting - Technical Design

## Architecture Overview

This change expands the built-in Markdown syntax definition so Markdown files highlight a broader set of explicit Markdown constructs instead of only a few headline-level spans.

The implementation keeps the existing syntax architecture:

- the Markdown grammar emits one tag per matched span
- the theme layer resolves tags to final styles through the existing hierarchical tag lookup
- fenced code blocks continue to inject nested syntax when the fence language resolves to a known syntax or alias

The main architectural goal is to make Markdown-specific structure visible without introducing a Markdown-only rendering path. Markdown remains a normal syntax definition driven by rules, spans, and tag-based theme lookup.

## Interface Design

### Markdown syntax rules

The Markdown syntax definition will continue to live in `src/syntax_builtin/markdown.toml`, but it will be expanded to cover common inline and block-level constructs.

Expected rule families include:

- headings tagged with `markup.heading`
- heading body text tagged with `markup.heading.text`
- emphasis tagged with `markup.emphasis`
- strong text tagged with `markup.strong`
- inline code tagged with `markup.code.inline`
- links tagged with `markup.link`
- link body text using the `.text` refinement when the region supports it
- lists tagged with `markup.list`
- list body text tagged with `markup.list.text` where useful
- blockquotes tagged with `markup.quote`
- blockquote body text tagged with `markup.quote.text` where useful
- fenced code blocks tagged with `markup.code`
- fence delimiters tagged as `punctuation`

The grammar should continue to use capture-based nested syntax for fenced code blocks so recognized fence languages can delegate to existing syntax definitions.

### Theme tag lookup

No new theme lookup mechanism is required. Markdown constructs will use the existing hierarchical lookup behavior:

- exact tag match first
- parent tag fallback next
- theme default style last

Markdown-specific tags such as `markup.heading`, `markup.link`, or `markup.code` will therefore work automatically in any theme that defines `markup` or a more specific refinement.

## Data Models

### Syntax definition

The Markdown syntax definition should remain a normal syntax file backed by TOML rules.

Changes to the syntax model are limited to the content of the Markdown rule set:

- more rules for explicit Markdown constructs
- more use of `markup.*` tags
- use of `.text` refinements where body text needs to be styleable independently from delimiters or markers

No new runtime syntax model is required.

### Theme styles

Built-in themes should include at least a base `markup` syntax style so Markdown constructs remain visibly distinct even when a theme does not define every child tag.

Recommended shipped refinements:

- `markup`
- `markup.heading`
- `markup.emphasis`
- `markup.strong`
- `markup.code.inline`
- `markup.link`
- `markup.list`
- `markup.quote`
- `markup.code`

Parent fallback should remain the mechanism that lets narrower tags inherit from `markup` when a theme does not define a more specific override.

## Key Components

### Markdown grammar loader

Responsibilities:

- parse the expanded Markdown rule set
- validate tags using the existing tag parser
- preserve fenced code block injection behavior
- reject malformed Markdown tag strings during startup

### Theme loader and built-in themes

Responsibilities:

- keep accepting hierarchical syntax tags in theme files
- provide shipped styles for Markdown markup tags
- let narrow Markdown tags fall back to broader `markup` styles when needed

### Buffer syntax tests

Responsibilities:

- verify Markdown fixtures highlight the intended constructs
- verify plain prose stays unstyled
- verify recognized fences still inject nested syntax
- verify unrecognized fences leave their body unstyled while preserving delimiter styling

## User Interaction

From the user’s perspective, Markdown files should become easier to scan:

- document structure should stand out
- prose should remain visually quiet
- fenced code blocks should still show language-aware highlighting when the fence language is known
- unrecognized fences should still look like fenced Markdown, just without injected body styling

## External Dependencies

This feature depends on existing internal systems only:

- syntax loading and tag validation
- theme loading and hierarchical style lookup
- built-in syntax registry for fence-language injection
- built-in theme definitions

No new external crates or services are required.

## Error Handling

Expected failure cases should continue to behave deterministically:

- malformed Markdown tag strings should fail during syntax loading
- unknown fence languages should fall back to unstyled fence bodies
- malformed Markdown rule definitions should fail fast at startup rather than partially rendering

The change should not introduce special recovery paths in the renderer.

## Security

This feature does not change editor security posture.

- Markdown parsing remains local and deterministic
- theme and syntax inputs continue to be validated at load time
- the feature does not add code execution or network access

## Configuration

No new user-facing configuration is required.

The feature is controlled by the existing syntax selection mechanism based on filetype detection and the existing theme selection mechanism.

## Component Interactions

```text
Markdown buffer text
  -> Markdown syntax rules match explicit constructs
  -> each match emits one markup-oriented tag
  -> fenced code rules delegate to a captured nested syntax when available
  -> renderer asks the active theme for the resolved style of each tag
  -> theme resolves exact tag, then parent tags, then default style
```

The Markdown grammar should stay focused on identifying structure, while themes decide the visual treatment of that structure.

## Platform Considerations

The implementation should remain portable across the editor’s supported terminals and operating systems.

- Markdown syntax rules should stay regex and delimiter based
- theme resolution should remain platform-neutral
- regression fixtures should use plain text Markdown content that behaves consistently across environments
