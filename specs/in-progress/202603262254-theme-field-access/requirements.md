# Theme Field Access Simplification

## Summary

Simplify the theme styling API so callers read UI and syntax style collections directly from `Theme` instead of using accessor methods. The direct fields should use the shorter names `ui` and `syntax`.

## Problem Statement

`Theme` currently exposes UI and syntax styles through separate accessor methods. That indirection adds noise at call sites and makes the theme object harder to read when the surrounding code already treats the style collections as plain data. A simpler field-based API would reduce boilerplate while preserving the same resolved styles.

## User Stories

- As a developer working on rendering, I want to read theme styles directly from the theme object, so that call sites stay concise and easy to scan.
- As a maintainer, I want the theme style collections to have shorter, clear names, so that the API is easier to understand across the codebase.
- As a contributor updating render paths, I want the theme style API to remain behaviorally unchanged, so that existing styling results do not change.

## Functional Requirements

- [ ] **REQ-001**: `Theme` must expose the resolved UI style collection directly under the name `ui`.
- [ ] **REQ-002**: `Theme` must expose the resolved syntax style collection directly under the name `syntax`.
- [ ] **REQ-003**: Callers must be able to read UI and syntax styles without using `ui_style()` or `syntax_style()` accessor methods.
- [ ] **REQ-004**: The direct `ui` and `syntax` fields must provide the same resolved style values that the current accessor methods return.
- [ ] **REQ-005**: Existing rendering and theme-loading behavior must continue to produce the same styles for all supported UI and syntax keys.
- [ ] **REQ-006**: The theme API must remain usable for existing functionality that depends on `default_style()` and `kind()`.
- [ ] **REQ-007**: Any code paths that previously used the UI or syntax accessors must be updated to the direct field-based API.
- [ ] **REQ-008**: The accessor methods for UI and syntax styles must no longer be part of the public theme API after the change.

## Non-Functional Requirements

- **Compatibility**: The change should be source-compatible only where callers already use the direct `Theme` object and should avoid altering rendered output.
- **Usability**: The resulting API should be shorter and more obvious at call sites.
- **Reliability**: Style lookups must remain deterministic and return the same values for a given theme definition.

## Acceptance Criteria

- [ ] **AC-001**: Rendering code can read a status bar style directly from `theme.ui` without calling a method.
- [ ] **AC-002**: Rendering code can read a syntax style directly from `theme.syntax` without calling a method.
- [ ] **AC-003**: Existing UI rendering continues to use the same tab, gutter, window, and status bar styles after the API change.
- [ ] **AC-004**: Existing syntax highlighting lookups continue to resolve to the same styles after the API change.
- [ ] **AC-005**: The codebase no longer relies on the removed UI and syntax accessor methods.

## Out of Scope

- Changing how themes are loaded from files
- Changing the resolved style values or palette resolution rules
- Renaming other `Theme` fields or changing `default_style()` / `kind()`
- Adding new style categories or theme concepts

## Assumptions

- The `Theme` type is the central place where rendering code reads resolved styles.
- Direct field access is acceptable for the theme style collections in this codebase.
- The existing style resolution logic should remain unchanged aside from the public API shape.

## Dependencies

- Existing `Theme` model definitions
- Existing UI style resolution logic
- Existing syntax style resolution logic
- Existing rendering call sites that consume theme styles
