# Editor Baseline Syntax Grammars

## Summary
Add proper lexical highlighting coverage for the baseline built-in language bundle identified in the syntax plan: `typescript`, `html`, `css`, `yaml`, `c`, `cpp`, `go`, and `java`.

## Problem Statement
Several built-in syntaxes that users commonly encounter while editing project files are currently metadata-only stubs or otherwise incomplete. This causes large portions of source files to render as plain text, which makes code harder to scan and can be misleading when filetype detection promises language-specific highlighting.

## User Stories
- As a user editing a TypeScript file, I want comments, strings, numbers, keywords, type syntax, and TS-specific declarations highlighted, so that code structure is easier to read.
- As a user editing HTML, I want tags, attributes, attribute values, comments, entities, and embedded script/style regions highlighted, so that markup remains readable.
- As a user editing CSS, I want selectors, at-rules, properties, values, numbers with units, colors, and strings highlighted, so that stylesheets are easier to navigate.
- As a user editing YAML, I want keys, scalars, block scalars, anchors, aliases, tags, directives, and punctuation highlighted, so that configuration files are easier to understand.
- As a user editing C, C++, Go, or Java, I want the common lexical elements of each language highlighted, so that everyday source code is not rendered as plain prose.

## Functional Requirements
- [ ] **REQ-001**: The `typescript` built-in shall highlight the language’s common lexical elements, including comments, strings, numbers, keywords, type syntax, interfaces, enums, decorators, and TypeScript-specific declarations.
- [ ] **REQ-002**: The `typescript` built-in shall support TSX files in a way that preserves recognizable JSX-like highlighting when `.tsx` remains mapped to TypeScript.
- [ ] **REQ-003**: The `html` built-in shall highlight tags, attributes, attribute values, comments, doctypes, and entities.
- [ ] **REQ-004**: The `html` built-in shall highlight embedded `<script>` and `<style>` bodies by delegating to the corresponding language syntax when those syntaxes are available.
- [ ] **REQ-005**: The `css` built-in shall highlight comments, selectors, at-rules, property names, property values, numbers with units, colors, strings, and punctuation.
- [ ] **REQ-006**: The `yaml` built-in shall highlight comments, keys, plain scalars, quoted scalars, block scalars, anchors, aliases, tags, directives, and punctuation.
- [ ] **REQ-007**: The `c` built-in shall highlight comments, preprocessor directives, strings, character literals, numeric literals with suffixes, keywords, builtin types, operators, and punctuation.
- [ ] **REQ-008**: The `cpp` built-in shall extend the C baseline with raw strings, namespaces, templates, boolean and null constants, and C++ keyword coverage.
- [ ] **REQ-009**: The `go` built-in shall highlight comments, raw strings, interpreted strings, rune literals, numbers, keywords, builtin types, and punctuation or operators.
- [ ] **REQ-010**: The `java` built-in shall highlight comments, doc comments, strings, text blocks, chars, numbers, annotations, keywords, and builtin types.
- [ ] **REQ-011**: Each language in this bundle shall preserve multiline lexical constructs that are expected to span lines, such as block comments, multiline strings, heredoc-like bodies, or block scalars.
- [ ] **REQ-012**: Each language in this bundle shall have regression fixture coverage that exercises comments, strings, multiline regions, and at least one language-specific construct.
- [ ] **REQ-013**: The new or improved grammars shall avoid relying on parser-only precision and shall remain compatible with the existing regex-and-region syntax engine.
- [ ] **REQ-014**: The new or improved grammars shall not broaden filetype behavior beyond the bundled languages unless required by an existing mapping already present in the codebase.

## Non-Functional Requirements
- [ ] **NFR-001**: The grammars shall be maintainable as focused language-specific definitions rather than a single mixed rule set.
- [ ] **NFR-002**: The highlighting behavior shall remain stable across line boundaries for multiline regions that are common in the supported languages.
- [ ] **NFR-003**: The implementation shall remain compatible with the editor’s existing syntax tag vocabulary and theme lookup behavior.
- [ ] **NFR-004**: The change shall not introduce unsafe code.

## Acceptance Criteria
- [ ] **AC-001**: Opening representative files for each of the eight bundled languages shows syntax highlighting for the expected lexical categories instead of plain text.
- [ ] **AC-002**: Multiline constructs in the supported grammars remain highlighted consistently when they cross line boundaries.
- [ ] **AC-003**: Embedded HTML script/style bodies, when present, render with the nested syntax highlighted rather than as undifferentiated text.
- [ ] **AC-004**: Regression fixtures exist for each bundled language and include the syntax cases required by this spec.
- [ ] **AC-005**: Existing supported grammars outside this bundle continue to load and highlight as before.

## Out of Scope
- Full parser-level accuracy for any of the bundled languages.
- Semantic highlighting, symbol resolution, type checking, or AST-driven classification.
- Introducing new filetype aliases beyond those already used by the editor’s syntax metadata.
- Reworking the syntax engine itself.

## Assumptions
- The current syntax engine’s regex, region, and injected-syntax features are sufficient for the requested baseline coverage.
- Existing filetype mappings for the targeted languages will remain in place unless a fixture or mapping inconsistency is discovered during implementation.
- The repo already has a fixture-based syntax test pattern that can be extended for these grammars.

## Dependencies
- Existing syntax tag and theme styling infrastructure.
- The current built-in syntax registry in `src/syntax/builtins`.
- Regression fixtures and tests for syntax highlighting behavior.
- The later implementation stage may need to rely on the existing `javascript`, `css`, and `html` grammars for nested injection behavior.
