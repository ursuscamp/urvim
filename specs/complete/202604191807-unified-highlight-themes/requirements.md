# Unified Highlight Themes

## Summary
urvim's theme system will be reworked into a single unified set of named highlights. The previous split between UI styles and syntax styles will be removed. All theme entries will use hierarchical names, with `ui.` reserved for editor chrome and `syntax.` reserved for buffer syntax highlighting. Style lookup will use exact-name resolution with parent fallback, matching the current syntax-tag behavior.

## Problem Statement
The current theme format uses two different style models:

- a closed set of fixed UI fields
- an open set of hierarchical syntax tags

This split makes theme files harder to read, less consistent, and harder to extend. It also forces special handling for some UI elements, even though they conceptually behave like other highlight names. The theme system should be simplified so every styled element is represented by the same kind of named highlight.

## User Stories
- As a theme author, I want to define UI and syntax styles using the same naming pattern, so that theme files are easier to understand and maintain.
- As a theme author, I want hierarchical names to fall back to parent names, so that I can define broad styles once and specialize only where needed.
- As a user, I want editor UI and syntax highlighting to keep looking correct after the theme system rewrite, so that the editor remains readable and consistent.
- As a maintainer, I want the built-in themes to use the same unified structure, so that future theme changes are simpler and more predictable.

## Functional Requirements
- [ ] **REQ-001**: Theme documents shall define all style entries in a single unified highlight namespace instead of separate UI and syntax sections.
- [ ] **REQ-002**: Theme highlight names shall support hierarchical parent fallback using dot-separated names, with the most specific defined highlight taking precedence.
- [ ] **REQ-003**: A highlight lookup shall return the nearest defined ancestor when an exact highlight name is not present in the theme.
- [ ] **REQ-004**: Highlight lookup shall not merge or overlay ancestor theme definitions into descendant theme definitions during resolution.
- [ ] **REQ-005**: Former UI styles shall be represented with `ui.`-prefixed highlight names.
- [ ] **REQ-006**: Former syntax styles shall be represented with `syntax.`-prefixed highlight names.
- [ ] **REQ-007**: The editor shall resolve former UI rendering locations, including active line styling, through the unified highlight lookup behavior.
- [ ] **REQ-008**: The active line highlight shall be usable without any special theme-section-specific handling.
- [ ] **REQ-009**: Built-in theme documents shall be rewritten to the unified highlight format and remain visually equivalent for existing editor states.
- [ ] **REQ-010**: Built-in theme documents shall keep UI-related and syntax-related highlight groups visually separated by comments for readability.
- [ ] **REQ-011**: Invalid highlight names in theme documents shall be rejected consistently with the existing theme validation behavior for hierarchical names.
- [ ] **REQ-012**: The theme system shall remain free of backward-compatibility support for the removed two-section format.
- [ ] **REQ-013**: The syntax documentation shall be updated to describe the unified highlight naming model and its fallback behavior.

## Non-Functional Requirements
- **Usability**: Theme files should be easier to scan and edit because all styled elements follow one naming convention.
- **Reliability**: Missing specific highlight names should degrade predictably to parent names or the default style, without partial style merging surprises.
- **Compatibility**: This change is an intentional breaking change for theme file format compatibility, but built-in themes and in-repo tests must be updated together.
- **Maintainability**: The theme model should avoid duplicated logic for UI and syntax style resolution.

## Acceptance Criteria
- [ ] **AC-001**: A theme file using only unified highlight names loads successfully and produces the expected styles for both UI and syntax rendering paths.
- [ ] **AC-002**: A child highlight such as `syntax.comment.todo` falls back to `syntax.comment` when the child is undefined.
- [ ] **AC-003**: A UI highlight such as `ui.active_line` falls back to `ui`-prefixed ancestors according to the same lookup rules used for syntax highlights.
- [ ] **AC-004**: A missing highlight name resolves to the theme default style when no ancestor highlight is defined.
- [ ] **AC-005**: The built-in themes render the same major UI regions and syntax categories as before the rewrite.
- [ ] **AC-006**: The built-in theme source files clearly group UI and syntax highlight definitions with comments.
- [ ] **AC-007**: Tests cover both direct matches and parent fallback for representative former UI styles and syntax styles.
- [ ] **AC-008**: The syntax documentation explains that theme highlights use the same hierarchical lookup rules for former UI names and syntax names.

## Out of Scope
- Backward compatibility with the old `[ui]` and `[syntax]` theme sections.
- Changes to syntax tokenization, syntax grammar definitions, or buffer parsing.
- Changes to rendering behavior outside of theme style lookup and application.
- New theme import/export tooling.

## Assumptions
- Existing built-in themes will be updated in the same change set as the theme model.
- The unified naming scheme will use `ui.` and `syntax.` prefixes for clarity.
- The current theme default style remains the base style used when no named highlight applies.
- The internal implementation may keep separate runtime representations if needed, as long as the public theme format is unified.

## Dependencies
- The current theme loader, resolver, and validation code.
- Built-in theme TOML files under `src/theme/builtin/`.
- Rendering call sites that currently access `theme.ui.*` or `theme.syntax_style_for_tag(...)`.
- Existing theme and rendering regression tests.
