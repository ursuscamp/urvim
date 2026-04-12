# Auto-Indent

## Summary
Add configurable auto-indent behavior for insert-mode newline creation. The editor should support an extensible auto-indent mode setting rather than a boolean flag, with one initial mode that uses nearby indentation when a new line is created.

## Problem Statement
Users expect new lines in code and structured text to preserve indentation in a predictable way. urvim currently has no dedicated auto-indent behavior, which makes insert-mode newline entry and open-line commands require manual spacing. The editor also needs a config surface that can grow beyond a single on/off switch so future indentation strategies can be added without redesigning the setting.

## User Stories
- As a user editing indented text, I want new lines to inherit useful indentation automatically, so that I do not have to retype leading whitespace.
- As a user working in mixed-indentation buffers, I want the editor to choose a sensible nearby indentation level, so that inserted lines stay aligned with surrounding content.
- As a user who prefers explicit control, I want to be able to disable auto-indent entirely, so that the editor behaves like plain text insertion when I want it to.
- As a user, I want the auto-indent setting to be extensible, so that future indentation styles can be added without replacing the config field.

## Functional Requirements
- [ ] **REQ-001**: The editor must support configuring auto-indent through a non-boolean setting that can represent multiple styles.
- [ ] **REQ-002**: The auto-indent setting must support an `off` state that disables auto-indent behavior.
- [ ] **REQ-003**: The auto-indent setting must support at least one enabled style that uses indentation from nearby buffer content.
- [ ] **REQ-004**: When auto-indent is disabled, newline creation must preserve existing plain-text behavior.
- [ ] **REQ-005**: When auto-indent is enabled, insert-mode newline creation must insert indentation derived from surrounding buffer content.
- [ ] **REQ-006**: Auto-indent inference must ignore blank lines when looking for nearby indentation.
- [ ] **REQ-007**: When a new line is created between two differently indented lines, the editor must choose the most-indented relevant neighbor as the source of indentation.
- [ ] **REQ-008**: Auto-indent must preserve the exact leading whitespace sequence from the chosen source line.
- [ ] **REQ-009**: If no usable indentation can be inferred, the editor must insert the new line without additional leading whitespace.
- [ ] **REQ-010**: Auto-indent behavior must apply to insert-mode `<Enter>` and to normal-mode open-line commands that create a new editable line.
- [ ] **REQ-011**: Auto-indent behavior must not change unrelated editing actions such as ordinary character insertion.
- [ ] **REQ-012**: The configuration schema must reject invalid auto-indent values at startup.

## Non-Functional Requirements
- [ ] **NFR-001**: The feature must remain backward compatible for users who leave auto-indent disabled.
- [ ] **NFR-002**: The implementation must remain predictable and local to the current buffer context, without requiring language-specific heuristics in the first version.
- [ ] **NFR-003**: The configuration surface must remain forward-compatible so additional auto-indent modes can be added later without changing the field into a boolean.
- [ ] **NFR-004**: The behavior must be testable through deterministic buffer state and cursor position assertions.

## Acceptance Criteria
- [ ] **AC-001**: With auto-indent disabled, pressing `<Enter>` in insert mode behaves like the current plain newline insertion.
- [ ] **AC-002**: With the neighbor auto-indent style enabled, pressing `<Enter>` in an indented line inserts a new line that begins with the expected indentation.
- [ ] **AC-003**: When a new line is created between two lines with different indentation, the resulting indentation matches the more-indented relevant neighbor.
- [ ] **AC-004**: When the surrounding buffer provides no indentation history, the inserted line contains no extra leading whitespace.
- [ ] **AC-005**: `o` and `O` create auto-indented blank lines consistent with insert-mode newline behavior.
- [ ] **AC-006**: Invalid auto-indent config values are rejected during startup configuration loading.
- [ ] **AC-007**: Future auto-indent modes can be added without changing the config field from its extensible non-boolean shape.

## Out of Scope
- Syntax-aware or filetype-aware indentation heuristics.
- Indentation based on brace matching, colon rules, or parser state.
- Retrofitting all buffer mutations or paste paths to auto-indent unless they already use the same newline-creation path.
- Visual indentation guides or rendering changes.

## Assumptions
- The initial behavior will be a neighbor style that uses nearby leading whitespace.
- The default config value will be `off` for compatibility.
- “Most-indented relevant neighbor” means the surrounding non-blank line with the greater leading-whitespace width.
- The feature will reuse existing editor and buffer editing primitives rather than introducing a new persistence layer.

## Dependencies
- The startup configuration system must support a new extensible auto-indent field.
- Insert-mode and open-line editing paths must be able to request indentation for newly created lines.
- Buffer inspection logic must be able to read nearby lines and measure leading whitespace.
