# Relative Line Numbering and Active Gutter Highlight - Implementation Tasks

## Overview
Implement a new relative line numbering toggle with absolute numbering on the cursor line, plus a dedicated active gutter row highlight that is carried by the built-in themes. The work touches config, theme resolution, window rendering, documentation, and regression tests.

## Backend
- [x] **1.** Add the `relative_number` config toggle to the startup config model and TOML schema.
  - [x] **1.1** Extend `Config` and `PartialConfig` with the new boolean field.
  - [x] **1.2** Resolve the field with a default of `false` in config loading.
  - [x] **1.3** Add config tests covering default resolution and TOML override behavior.
  - [x] **1.4** Update `docs/config.md` with the new setting. (depends on: 1.1)

- [x] **2.** Extend the theme schema and resolved theme model with a dedicated active gutter row style.
  - [x] **2.1** Add the new gutter active-line style key to the theme key/model layer.
  - [x] **2.2** Update theme loading so built-in and custom themes can resolve the new style.
  - [x] **2.3** Add model and loader tests for the new style field and schema coverage.

## Rendering
- [x] **3.** Update gutter rendering to support relative line numbers and active-row styling.
  - [x] **3.1** Render the cursor line with its absolute buffer line number when relative numbering is enabled.
  - [x] **3.2** Render other visible logical lines as their positive distance from the cursor line when relative numbering is enabled.
  - [x] **3.3** Preserve the existing blank continuation-row behavior for wrapped lines.
  - [x] **3.4** Apply the dedicated active gutter row style across the full gutter width on the cursor line.
  - [x] **3.5** Keep gutter width and content offset calculations unchanged apart from the new labels.

- [x] **4.** Thread the new gutter behavior through the window render path.
  - [x] **4.1** Pass the current cursor line and render-state gating needed by the gutter renderer.
  - [x] **4.2** Keep relative numbering available in normal, insert, and visual modes.
  - [x] **4.3** Preserve the existing focused-window and active-line gating rules for the active gutter highlight.
  - [x] **4.4** Add render tests that verify absolute-line fallback, relative numbering, and active-row styling.

## Theme Assets
- [x] **5.** Update the built-in themes to define the new active gutter row style.
  - [x] **5.1** Choose a readable but subtle background for each built-in theme's active gutter row.
  - [x] **5.2** Keep the style visually consistent with the existing gutter and active-line palette choices.

## Testing
- [x] **6.** Run the relevant test suite and fix any regressions introduced by the change.
  - [x] **6.1** Add or update unit tests in config, theme, and window modules.
  - [x] **6.2** Run `cargo fmt` after implementation changes.
  - [x] **6.3** Run `cargo check` for the workspace and the targeted test modules.
  - [x] **6.4** Address any clippy warnings surfaced by the touched code paths.

## Completion Summary
| Section | Total | Done | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 2 | 2 | 0 |
| Rendering | 2 | 2 | 0 |
| Theme Assets | 1 | 1 | 0 |
| Testing | 1 | 1 | 0 |
| Total | 6 | 6 | 0 |
