# Editor Baseline Syntax Grammars - Technical Design

## Architecture Overview
This change extends the built-in syntax registry with real lexical grammars for the baseline editor languages identified in `syntax_plan.md`: `typescript`, `html`, `css`, `yaml`, `c`, `cpp`, `go`, and `java`.

The implementation should follow the existing syntax engine model:
- syntax definitions remain declarative and filetype-driven
- highlighting is expressed through regex rules, delimited regions, and nested rule targets
- multiline constructs are preserved through regions that span lines
- language injection is used only where the current engine already supports it

The goal is to make these built-ins visibly correct for everyday editing without requiring parser-level accuracy.

## Interface Design
The public-facing interfaces do not change. The work is consumed through existing filetype resolution and buffer syntax highlighting APIs.

Relevant behavior:
- opening a buffer with one of the target filetypes should resolve to the corresponding built-in syntax
- syntax spans produced by `Buffer::syntax_spans_for_line` should include the expected tags for lexical categories
- syntax metadata should continue to expose the same language names and filename mappings already used by the editor

No new end-user configuration surface is required for this feature.

## Data Models
The change primarily updates syntax-definition data and test fixtures rather than editor runtime data structures.

Updated data artifacts:
- built-in syntax TOML definitions for each targeted language
- per-language syntax regression fixtures under `src/buffer/tests/syntax/fixtures/`
- per-language syntax tests under `src/buffer/tests/syntax/`

Expected lexical tag families:
- `comment` and `comment.documentation`
- `string`
- `number`
- `keyword`
- `type`
- `variable`
- `constant`
- `operator`
- `punctuation`
- `markup`, `markup.tag`, `markup.attribute`, or equivalent tag vocabulary where already supported

## Key Components

### Built-in Syntax Definitions
Each targeted language should have a focused syntax definition that covers the common lexical forms required by the requirements document.

Responsibilities:
- classify the common lexical categories for the language
- preserve multiline regions such as block comments, text blocks, block scalars, or raw string bodies
- keep unsupported edge cases unhighlighted rather than trying to infer semantics

### Nested Syntax Injection
The HTML grammar should delegate script and style bodies when the nested language is available and the current syntax registry can resolve it.

Responsibilities:
- select the nested syntax by opener text or a fixed mapping where appropriate
- fall back safely when the nested syntax is unavailable or intentionally unsupported

### Syntax Regression Fixtures
Each language should have representative fixture content that exercises the grammar’s intended coverage.

Responsibilities:
- provide stable line-oriented examples for comments, strings, numbers, and multiline constructs
- include at least one language-specific construct that distinguishes the grammar from a generic baseline
- support tests that assert on line-level spans and tag families

### Syntax Test Modules
The per-language test modules should validate that the built-in syntax definitions are actually producing the intended tags.

Responsibilities:
- load the fixture file for the language
- assert on specific lines and spans for representative tokens
- cover both common lexical cases and the language-specific construct called out in the requirements

## User Interaction
There is no new direct UI flow.

The user experience changes when:
- a file is opened with one of the supported filetypes
- the buffer renderer requests syntax spans for the visible lines

Expected outcome:
- code, markup, and configuration files become readable at a glance
- multiline constructs remain visually coherent across line boundaries
- HTML embedded bodies are easier to read because nested syntax is visible

## External Dependencies
This feature depends on the existing editor syntax engine and the current theme tag vocabulary.

External or shared dependencies:
- the built-in syntax registry and filetype matcher
- the tag/style resolution system in the theme layer
- the existing syntax test helpers in `src/buffer/tests.rs`
- any already-supported nested syntaxes used by HTML injection, especially `javascript` and `css`

## Error Handling
Expected failure modes should be handled conservatively:

- invalid regexes in syntax definitions must fail during syntax loading
- unknown nested syntax references must be rejected by the loader
- unsupported nested injection should fall back to the host region style instead of breaking highlighting
- unsupported lexical edge cases should remain plain text rather than causing misclassification

The implementation should prefer incomplete but stable highlighting over fragile heuristics.

## Security
The feature does not introduce new security-sensitive behavior.

Relevant constraints:
- syntax definitions are data-driven and should not execute code
- no new secrets, credentials, or network access are required
- regex rules should remain bounded to the existing loader and runtime patterns

## Configuration
No new configuration options are required.

The feature should respect existing editor settings:
- syntax highlighting remains controlled by the current syntax-enable behavior
- theme styling continues to map tags to styles as it does today

## Component Interactions
1. Filetype detection selects one of the target built-in syntaxes.
2. The syntax loader compiles the relevant rules and resolves any nested references.
3. Buffer highlighting requests spans line by line from the loaded syntax.
4. The renderer consumes the spans and applies the active theme styles.
5. Regression fixtures verify the expected output at the syntax-span level.

## Platform Considerations
The feature should remain portable across platforms supported by the editor.

Key considerations:
- line-ending differences should not affect syntax region behavior
- path-based fixture loading must continue to work with absolute paths and test helpers
- regex and region behavior should not depend on platform-specific shell or filesystem semantics
