# Nord Theme

## Summary
Add a new built-in Nord-inspired theme for urvim, based as closely as practical on `shaunsingh/nord.nvim`, and wire it into the editor as a selectable theme alongside the existing built-ins.

## Problem Statement
urvim already ships several built-in themes, but it does not include a Nord option. Users who prefer the Nord palette cannot select it without defining a custom theme, and the project lacks a faithful built-in Nord variant that matches the rest of the built-in theme system.

## User Stories
- As a user, I want to select a built-in Nord theme, so that I can use a familiar calm dark palette without custom setup.
- As a maintainer, I want the Nord theme to behave like the other built-ins, so that theme loading and validation stay consistent.
- As a contributor, I want tests and docs updated with the new theme, so that the new option is discoverable and protected from regressions.

## Functional Requirements
- [ ] **REQ-001**: The editor shall include a new built-in theme named `Nord`.
- [ ] **REQ-002**: The Nord theme shall be selectable through the existing `theme` configuration and `--theme` CLI override mechanisms.
- [ ] **REQ-003**: The Nord theme shall be loaded from a built-in TOML theme definition rather than hardcoded renderer logic.
- [ ] **REQ-004**: The Nord theme shall use a palette and highlight mapping that is faithful to `shaunsingh/nord.nvim` as closely as urvim's theme model allows.
- [ ] **REQ-005**: The Nord theme shall define a dark default background and foreground consistent with the upstream Nord palette.
- [ ] **REQ-006**: The Nord theme shall provide UI highlight styles for the same core editor surfaces covered by the existing built-in themes, including windows, gutters, tabs, selection, prompts, notifications, and active-line styling.
- [ ] **REQ-007**: The Nord theme shall provide syntax highlight styles for the same core syntax categories covered by the existing built-in themes.
- [ ] **REQ-008**: Theme registry loading shall include the Nord theme without changing the behavior of existing built-in themes.
- [ ] **REQ-009**: Existing theme selection behavior shall continue to fail clearly for unknown theme names.
- [ ] **REQ-010**: Documentation that lists or describes built-in themes shall mention the new Nord theme.
- [ ] **REQ-011**: Automated tests shall cover Nord theme registration and selection.
- [ ] **REQ-012**: Automated tests shall cover at least one representative Nord style resolution path to guard against palette or mapping regressions.

## Non-Functional Requirements
- **Consistency**: Nord must fit the same theme schema and loading pipeline as the other built-in themes.
- **Faithfulness**: The color choices should track the upstream Nord palette and highlight intent where urvim has equivalent surfaces.
- **Maintainability**: Theme-specific values should live in theme assets or theme-loading code, not spread across unrelated UI modules.

## Acceptance Criteria
- [ ] **AC-001**: `--theme Nord` activates the new built-in theme.
- [ ] **AC-002**: The built-in theme registry reports Nord among the available theme names.
- [ ] **AC-003**: The Nord theme renders core UI surfaces with Nord-style colors rather than falling back to the default theme.
- [ ] **AC-004**: Theme docs or config docs mention Nord as an available built-in theme.
- [ ] **AC-005**: Tests fail if Nord is removed from the registry or if its selected styles regress materially.

## Out of Scope
- Adding runtime Nord-specific user options such as transparent background, bold toggles, italic toggles, or diff background modes from the upstream Neovim theme.
- Reworking urvim's theme model to support per-theme runtime configuration flags.
- Adding support for external plugin highlight groups that urvim does not currently model.
- Changing the default built-in theme away from `Friday Night`.

## Assumptions
- Urvim's existing theme schema can express a faithful-enough Nord port without adding new theme fields.
- The current UI surfaces and syntax tags are sufficient to represent the most important Nord mappings.
- The project prefers a single canonical built-in Nord variant rather than multiple Nord flavors.

## Dependencies
- Theme registry and theme loader infrastructure.
- Built-in theme asset loading.
- Theme-related configuration and startup selection code.
- Existing theme and config documentation.
- Test coverage for theme registry and UI style resolution.
