# Todo Comment Highlighting

## Summary
urvim should visually highlight task markers such as `TODO` and `FIXME` when they appear inside comments. The marker set should have sensible defaults, remain user-configurable, and draw from theme-defined styles so different marker types can be distinguished visually.

## Problem Statement
Comments often contain follow-up work, known issues, and reminders that are easy to miss when they render like ordinary comment text. The editor already understands syntax-aware comment regions, but it does not yet give these markers a distinct visual treatment. As a result, important notes buried in comments are harder to spot while reading or reviewing code.

## User Stories
- As a programmer, I want `TODO` and similar markers in comments to stand out, so that I can quickly find pending work.
- As a user with personal marker conventions, I want to customize which markers are highlighted, so that the editor matches my workflow.
- As someone who cares about theme consistency, I want each marker type to use theme-provided styling, so that the highlighting fits the active theme.

## Functional Requirements
- [ ] **REQ-001**: The editor shall highlight task markers when they appear inside comment text recognized by the current syntax system.
- [ ] **REQ-002**: The editor shall use the default marker set `TODO`, `FIXME`, `BUG`, and `NOTE` when no custom marker list is configured.
- [ ] **REQ-003**: The editor shall allow the marker list to be customized through configuration.
- [ ] **REQ-004**: Marker matching shall be case-sensitive.
- [ ] **REQ-005**: Marker matching shall only apply to standalone words and shall not match inside longer tokens.
- [ ] **REQ-006**: The editor shall provide separate theme-style hooks for each configured marker type.
- [ ] **REQ-007**: The editor shall use the theme-defined style for a marker type when that style is available.
- [ ] **REQ-008**: If a theme does not define a style for a marker type, the editor shall fall back to a safe non-breaking default style.
- [ ] **REQ-009**: Highlighting task markers shall not modify buffer contents, comment syntax metadata, or file contents on disk.

## Non-Functional Requirements
- [ ] **NFR-001**: Marker highlighting shall remain responsive during normal editing and scrolling.
- [ ] **NFR-002**: The feature shall remain compatible with existing syntax highlighting behavior for non-comment text.
- [ ] **NFR-003**: The feature shall remain usable without any custom configuration.

## Acceptance Criteria
- [ ] **AC-001**: A comment containing `TODO`, `FIXME`, `BUG`, or `NOTE` shows each marker with a distinct style provided by the active theme.
- [ ] **AC-002**: A custom marker list is honored by the editor, and when configured it replaces the default marker list for highlighting.
- [ ] **AC-003**: Lowercase or mixed-case variants such as `todo` or `Todo` are not highlighted.
- [ ] **AC-004**: Substrings inside longer words such as `TODOLIST` or `FIXME123` are not highlighted.
- [ ] **AC-005**: Text outside comments does not receive todo-marker highlighting.
- [ ] **AC-006**: If a theme omits one or more marker-specific styles, the editor still renders comment text normally without breaking layout or editing behavior.

## Out of Scope
- Detecting or highlighting arbitrary issue IDs, usernames, or URLs inside comments.
- Automatically creating tasks, diagnostics, or jump targets from highlighted markers.
- Parsing semantic meaning beyond literal marker matching.
- Changing comment syntax detection itself.

## Assumptions
- Existing syntax highlighting already identifies comment regions accurately enough for marker highlighting to build on top of it.
- Configuration will expose a marker list in a way that can be extended later without changing the matching rules.
- Theme support for marker-specific styles will be additive and should not require breaking existing themes.
- The default marker list is intended as a baseline that users can replace or expand.

## Dependencies
- Existing syntax-aware comment detection.
- Existing theme styling infrastructure.
- Existing configuration loading and persistence.
- Existing buffer rendering pipeline.
