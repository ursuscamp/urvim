# Unified Highlight Themes - Implementation Tasks

## Overview
Rework the theme system into one unified hierarchical highlight model, update all render call sites, rewrite the built-in themes, and refresh the syntax documentation.

## Backend
- [x] **1.** Redesign the raw and resolved theme models around a single unified highlight map.
  - [x] **1.1** Replace the split raw `ui` / `syntax` schema with a single highlights table keyed by hierarchical names.
  - [x] **1.2** Replace the resolved UI-specific theme storage with one unified resolved highlight collection.
  - [x] **1.3** Preserve the theme default style and palette resolution behavior.

- [x] **2.** Update theme resolution and lookup to use uniform parent fallback for all highlight names.
  - [x] **2.1** Resolve highlight names with the existing hierarchical tag parser and reject invalid names.
  - [x] **2.2** Make exact-match lookup fall back to parent names without merging ancestor styles.
  - [x] **2.3** Expose a single theme lookup API suitable for both former UI names and syntax names.

- [x] **3.** Update rendering consumers to request unified highlight names instead of reading split UI fields.
  - [x] **3.1** Convert window rendering paths to resolve the buffer, gutter, active line, selection, tab, and split styles through unified highlight lookup.
  - [x] **3.2** Convert the status bar and other UI consumers to use the new `ui.*` naming scheme.
  - [x] **3.3** Preserve current style composition order when overlaying highlights during rendering.

## Content
- [x] **4.** Rewrite the built-in theme TOML files to the unified format.
  - [x] **4.1** Replace the old section split with a single highlight table in each builtin theme.
  - [x] **4.2** Rename former UI styles to `ui.*` names and syntax styles to `syntax.*` names.
  - [x] **4.3** Add comments in the builtin files to visually separate UI and syntax highlight groups.

- [x] **5.** Update syntax documentation for the unified highlight model.
  - [x] **5.1** Revise `docs/syntax/highlighting.md` to explain unified highlight naming and fallback behavior.
  - [x] **5.2** Revise `docs/syntax/tags.md` so the tag vocabulary and resolution rules reflect the unified highlight model.

## Testing
- [x] **6.** Add and update regression tests for the new theme model and rendering behavior.
  - [x] **6.1** Cover unified theme parsing and resolution for representative UI and syntax names.
  - [x] **6.2** Cover parent fallback for both `ui.*` and `syntax.*` highlight names.
  - [x] **6.3** Update rendering tests that previously asserted split `theme.ui` behavior.
  - [x] **6.4** Run `cargo check` and the relevant test suite after the refactor.

## Completion Summary
| Area | Status | Notes |
|---|---|---|
| Backend | Done | Theme model, resolver, and render call sites now use unified highlight lookup. |
| Content | Done | Built-in themes and syntax docs were rewritten for the new format. |
| Testing | Done | Regression coverage was updated and the full test suite passes. |
