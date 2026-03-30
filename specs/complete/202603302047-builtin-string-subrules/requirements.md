# Builtin String Regions

## Summary
urvim should highlight escape sequences and string interpolation in builtin syntax definitions that support those string forms, using delimited regions for all string forms so nested code and escaped characters are styled correctly.

## Problem Statement
Several builtin grammars still treat strings as a special schema shape rather than as regular delimited regions, even when the language has meaningful escape sequences or nested interpolation forms. That leaves important structure visually merged into the surrounding string text and makes it harder to read language-specific string syntax at a glance. The builtin syntax files should use the refined syntax format to declare all string forms as delimited regions consistently.

## User Stories
- As a programmer, I want escape sequences inside builtin language strings to be highlighted distinctly, so that special characters and formatting codes are easy to spot.
- As a programmer, I want interpolation inside builtin language strings to be highlighted distinctly, so that embedded expressions read like code instead of plain text.
- As a maintainer, I want builtin grammars to express string escapes and interpolation through delimited regions, so that language-specific string behavior stays close to the host definition.
- As someone editing multiple supported languages, I want string highlighting to behave consistently across builtins, so that I can rely on the same visual patterns across the editor.

## Functional Requirements
- [ ] **REQ-001**: The editor shall highlight escape sequences in builtin string literals using syntax-aware styling distinct from the surrounding string text.
- [ ] **REQ-002**: The editor shall highlight string interpolation bodies in builtin string literals using syntax-aware styling distinct from the surrounding string text.
- [ ] **REQ-003**: Builtin syntax definitions shall represent all string forms as delimited regions rather than as a separate string-spec schema.
- [ ] **REQ-004**: Builtin syntax definitions that support escape sequences shall attach an appropriate nested rule set to the string region rather than relying on hardcoded renderer logic.
- [ ] **REQ-005**: Builtin syntax definitions that support interpolation shall attach an appropriate nested rule set to the string region rather than relying on hardcoded renderer logic.
- [ ] **REQ-006**: String escape and interpolation handling shall remain language-specific, so builtin grammars only enable the behavior where the source language actually supports it.
- [ ] **REQ-007**: String escape and interpolation highlighting shall preserve the surrounding region styling before, inside, and after nested spans.
- [ ] **REQ-008**: Multiline string regions that support nested sub-rules shall preserve highlight state across line boundaries.
- [ ] **REQ-009**: Languages and string forms that do not define escape or interpolation rules shall continue to render as plain delimited regions.
- [ ] **REQ-010**: String escape and interpolation highlighting shall not alter buffer contents, cursor movement, undo behavior, or save behavior.
- [ ] **REQ-011**: Builtin syntax definitions using nested string regions shall continue to validate and load successfully at startup.

## Non-Functional Requirements
- **Compatibility**: The feature shall work with the current syntax registry, syntax cache, and theme style categories.
- **Reliability**: Highlighting shall remain correct after edits inside or adjacent to nested string regions.
- **Maintainability**: String escape and interpolation behavior shall live in the syntax definitions for each builtin language, not in duplicated special-case code paths.
- **Usability**: Escapes and interpolation shall be visually distinguishable without user configuration.

## Acceptance Criteria
- [ ] **AC-001**: A builtin language file with escape-sequence support shows escape text with distinct syntax styling while preserving the surrounding region styling.
- [ ] **AC-002**: A builtin language file with interpolation support shows the embedded expression with non-string highlighting while preserving the surrounding region styling.
- [ ] **AC-003**: A multiline string region that contains supported nested sub-rules continues to highlight correctly after line breaks until the region closes.
- [ ] **AC-004**: A builtin syntax definition that declares nested string regions loads without validation errors.
- [ ] **AC-005**: A builtin language without escape or interpolation rules continues to render its string regions normally.

## Out of Scope
- Parser-backed highlighting engines such as tree-sitter.
- User-defined syntax rules or custom runtime grammar configuration.
- Semantic highlighting from language servers.
- New theme categories dedicated to escapes or interpolation.
- Non-string nested syntax features that are unrelated to builtin string handling.

## Assumptions
- The refined syntax format already supports nested rule sets and injected syntax targets for string bodies.
- Only builtin syntax definitions in the repository scope are part of this work.
- The active theme already provides sufficient existing style categories for string text, punctuation, and embedded syntax.
- The first implementation can focus on the builtin languages that already have enough grammar information to express these string behaviors.

## Dependencies
- Existing syntax-definition loading and validation.
- Existing nested rule-set and injected-syntax support in the syntax engine.
- Existing syntax fixtures and regression tests for builtin grammars.
- Existing theme syntax style categories.
