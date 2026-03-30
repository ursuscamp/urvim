# Syntax Highlighting

## Summary
urvim should visually distinguish source code and structured text using syntax-aware colors and text attributes that come from the active theme. The highlighting should be owned by the buffer so it updates as the buffer changes and stays consistent across all views of the same buffer.

## Problem Statement
At the moment, buffer text is rendered as plain themed text without syntax-specific styling. That makes code harder to scan, especially for languages with clear structure such as Rust, Python, JavaScript, shell scripts, JSON, TOML, and Markdown. The editor already has filetype detection and theme syntax colors, so the missing piece is a buffer-backed highlighting system that keeps the rendered view in sync with edits.

## User Stories
- As a programmer, I want source code to highlight keywords, strings, comments, and related syntax categories, so that I can read code more quickly.
- As a writer or editor of structured text, I want Markdown, JSON, and TOML to be easier to scan, so that I can spot structure and mistakes faster.
- As someone editing a file in multiple windows, I want the same buffer to highlight consistently in each view, so that the display does not depend on a single window.

## Functional Requirements
- [ ] **REQ-001**: The editor shall render buffer text with syntax-aware styling for supported filetypes.
- [ ] **REQ-002**: Syntax highlighting shall be owned by the buffer so that all views of the same buffer share the same highlighted content state.
- [ ] **REQ-003**: Syntax highlighting shall update when the underlying buffer text changes, including insertions, deletions, line joins, line splits, and undo/redo.
- [ ] **REQ-004**: The editor shall use the active theme's syntax styles for highlighted spans.
- [ ] **REQ-005**: If a buffer's filetype is unsupported, the editor shall render the buffer as plain themed text without syntax-specific styling.
- [ ] **REQ-006**: The highlighting system shall support at least the initial core set of filetypes: Rust, Python, JavaScript, TypeScript, Shell, JSON, TOML, and Markdown.
- [ ] **REQ-007**: The highlighting system shall preserve continuation state across line boundaries for syntax constructs that span multiple lines.
- [ ] **REQ-008**: Highlighting shall not alter buffer contents, cursor position, undo history semantics, or save behavior.
- [ ] **REQ-009**: The editor shall continue to render correctly when a theme provides only the existing syntax style slots.

## Non-Functional Requirements
- [ ] **NFR-001**: Highlighting updates shall remain responsive during normal editing and scrolling.
- [ ] **NFR-002**: The highlighting system shall be reliable under repeated edits, including edits near the start of the file and edits that affect multiple lines.
- [ ] **NFR-003**: The feature shall remain compatible with the current theme system and filetype detection logic.
- [ ] **NFR-004**: The feature shall be usable without any additional user configuration.

## Acceptance Criteria
- [ ] **AC-001**: Opening a Rust, Python, JavaScript/TypeScript, Shell, JSON, TOML, or Markdown file shows visibly distinct syntax categories instead of uniform text styling.
- [ ] **AC-002**: Editing highlighted text updates the displayed syntax without requiring a restart or manual refresh.
- [ ] **AC-003**: Multiple windows showing the same buffer display the same syntax highlighting state.
- [ ] **AC-004**: Unsupported filetypes still render normally with no syntax-specific colors.
- [ ] **AC-005**: Existing behavior for gutter rendering, cursor movement, save, and undo/redo remains intact after highlighting is enabled.

## Out of Scope
- Parser-backed highlighting engines such as tree-sitter.
- User-defined syntax rules or custom grammar configuration.
- Semantic highlighting from language servers.
- Diagnostics, error underlining, or lint annotations.
- Per-project highlight configuration beyond the current theme and filetype detection.

## Assumptions
- The active theme already provides sufficient syntax style slots for the first release.
- Filetype detection remains the mechanism for choosing which syntax rules apply to a buffer.
- Syntax highlighting is derived data and does not need to be saved to disk.
- The first version can focus on the core filetypes listed above before expanding to the full `Filetype` enum.

## Dependencies
- Existing buffer filetype detection.
- Existing theme syntax styles.
- Existing render pipeline for window content.
- Existing buffer mutation and undo/redo paths.
