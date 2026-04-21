# Indent Guides - Implementation Tasks

## Overview
Implement a cursor-active indent guide overlay backed by the existing indent scope cache, controlled by a new `indent_guides` config flag (default enabled), with ASCII fallback and Unicode line-style rendering when `unicode_indent` capability is available.

## Backend
- [x] **1.** Add `indent_guides` configuration support with default-on behavior.
  - [x] **1.1** Extend config data model and parser with `indent_guides: bool`.
  - [x] **1.2** Set default to `true` when omitted.
  - [x] **1.3** Add validation coverage for type/parse behavior consistent with existing config flags.
  - [x] **1.4** Document `indent_guides` in `docs/config.md`. (depends on: 1.1)

- [x] **2.** Implement active indent scope selection for the cursor.
  - [x] **2.1** Read cursor line containing-scope ids from the indent scope cache.
  - [x] **2.2** Compute cursor visual column using existing tab-expanded column logic.
  - [x] **2.3** Select the deepest scope with indent column `<=` cursor visual column. (depends on: 2.1, 2.2)
  - [x] **2.4** Derive interior render bounds and return no guide when interior is empty. (depends on: 2.3)
  - [x] **2.5** End the guide at the line immediately before the first shallower-indent line (via scope boundary semantics). (depends on: 2.3)

- [x] **3.** Integrate guide overlay into window rendering.
  - [x] **3.1** Gate rendering on `indent_guides` config.
  - [x] **3.2** Render a single vertical guide only on interior lines (exclude opening/closing lines).
  - [x] **3.3** Keep the guide continuous across blank lines inside the selected scope.
  - [x] **3.4** Ensure rendering does not overwrite or alter source characters on excluded boundary lines.

- [x] **4.** Add capability-aware glyph selection.
  - [x] **4.1** Use ASCII `|` when `unicode_indent` capability is unavailable.
  - [x] **4.2** Reuse existing Unicode line-drawing style glyph selection when `unicode_indent` is available.

## Testing
- [x] **5.** Add regression tests for active guide selection and rendering.
  - [x] **5.1** Test deepest-eligible scope selection at/before cursor visual column.
  - [x] **5.2** Test mixed tabs/spaces lines to confirm visual-column behavior.
  - [x] **5.3** Test no-guide behavior when no eligible scope exists.
  - [x] **5.4** Test no-guide behavior when selected scope has no interior lines.
  - [x] **5.5** Test guide continuity across blank interior lines.
  - [x] **5.6** Test boundary exclusion (guide absent on opening/closing lines).
  - [x] **5.7** Test config-off behavior (`indent_guides = false`).
  - [x] **5.8** Test ASCII fallback glyph (`|`) when Unicode capability is absent.
  - [x] **5.9** Test Unicode glyph path uses existing line-drawing style when capability is present.

- [x] **6.** Verify project quality gates.
  - [x] **6.1** Run `cargo fmt` after implementation changes.
  - [x] **6.2** Run targeted tests for rendering/scope logic.
  - [x] **6.3** Run full `cargo check` and address warnings.

## Completion Summary
| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Backend | 4 | 4 | Done |
| Testing | 2 | 2 | Done |
| Total | 6 | 6 | Done |
