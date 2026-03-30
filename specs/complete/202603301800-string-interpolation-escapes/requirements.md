# String Interpolation and Escape Codes

## Summary
urvim should highlight string interpolation and escape codes in languages that use them, so that string literals remain readable and nested expressions or escape sequences are styled correctly within the host language's syntax rules.

## Problem Statement
Current syntax highlighting treats strings as a mostly uniform span, which leaves interpolated expressions and escape sequences insufficiently distinguished in languages that embed additional syntax inside string literals. This makes mixed-content strings harder to scan and can hide important structure such as embedded expressions, format placeholders, and escape sequences. The syntax system needs a way to represent these nested string constructs as part of the host language grammar so they highlight consistently without requiring a separate grammar file for every inner fragment.

## User Stories
- As a programmer, I want interpolated expressions inside strings to be highlighted distinctly, so that I can read embedded code without losing track of the surrounding string.
- As a programmer, I want escape sequences inside strings to be highlighted distinctly, so that I can recognize special characters and formatting codes at a glance.
- As a maintainer, I want interpolation and escape handling to live in the host language syntax definition, so that related string behavior stays close to the grammar that uses it.
- As someone editing files with multiple nested string forms, I want highlighting to remain correct across multiline strings and nested regions, so that editing does not break the displayed structure.

## Functional Requirements
- [ ] **REQ-001**: The editor shall highlight string interpolation constructs inside supported string literals using syntax-aware styling distinct from the surrounding string text.
- [ ] **REQ-002**: The editor shall highlight escape sequences inside supported string literals using syntax-aware styling distinct from the surrounding string text.
- [ ] **REQ-003**: String interpolation shall be represented as a nested syntax region within the host language grammar rather than as a separate standalone grammar file.
- [ ] **REQ-004**: Escape-sequence handling shall be defined within the host language grammar for each supported filetype rather than requiring a separate grammar file for escape codes alone.
- [ ] **REQ-005**: Nested interpolation regions shall preserve the active syntax context of the interpolated expression until the interpolation closes.
- [ ] **REQ-006**: Syntax highlighting shall continue to render the surrounding string literal correctly before, inside, and after interpolation or escape regions.
- [ ] **REQ-007**: Multiline string literals that contain interpolation or escape sequences shall preserve their highlight state across line boundaries.
- [ ] **REQ-008**: Unsupported filetypes or string forms without interpolation and escape rules shall continue to render as plain string text without nested syntax styling.
- [ ] **REQ-009**: String interpolation and escape highlighting shall not alter buffer contents, cursor movement, undo behavior, or save behavior.
- [ ] **REQ-010**: Syntax definitions that enable string interpolation or escape handling shall continue to use the active theme's existing syntax style categories.

## Non-Functional Requirements
- **Compatibility**: The feature shall work with the current theme system and existing syntax-highlighting pipeline.
- **Reliability**: Highlighting shall remain correct under repeated edits inside or adjacent to interpolated strings and escape sequences.
- **Maintainability**: Host language grammars shall remain the source of truth for interpolation and escape behavior, so related syntax rules stay localized.
- **Usability**: Interpolated expressions and escape codes shall be visually distinguishable without requiring user configuration.

## Acceptance Criteria
- [ ] **AC-001**: A supported language file containing string interpolation shows the embedded expression with non-string highlighting while preserving the surrounding string styling.
- [ ] **AC-002**: A supported language file containing escape sequences shows the escape text with distinct syntax-aware styling while preserving the surrounding string styling.
- [ ] **AC-003**: A multiline interpolated string continues to highlight correctly across line breaks until the string or interpolation closes.
- [ ] **AC-004**: Editing text inside or before an interpolated string refreshes the displayed highlighting without requiring a restart or manual refresh.
- [ ] **AC-005**: Languages that do not define interpolation or escape rules continue to render string literals normally.

## Out of Scope
- Parser-backed highlighting engines such as tree-sitter.
- User-defined syntax rules or custom grammar configuration.
- Semantic highlighting from language servers.
- General-purpose string parsing that is not described by the host language syntax definition.
- New theme categories specifically for interpolation or escape codes.

## Assumptions
- Supported languages already have syntax definitions that can describe string regions and nested syntax regions.
- The active theme already provides sufficient existing syntax style slots for string, punctuation, and embedded syntax categories.
- String interpolation and escape handling can be expressed with the current syntax-region model.
- The first version can focus on the languages already covered by the existing syntax system rather than every possible language.

## Dependencies
- Existing syntax-definition loading and validation.
- Existing syntax-region nesting and incremental highlighting behavior.
- Existing theme syntax style categories.
- Existing filetype detection and host-language syntax selection.
