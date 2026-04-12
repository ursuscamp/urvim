# Auto-Indent - Implementation Tasks

## Overview
Implement extensible auto-indent support with an initial `neighbor` mode that reuses nearby indentation when creating new lines. The work is split so config, inference, editor behavior, and tests can be completed independently and verified in small steps.

## Backend
- [x] **1.** Add an extensible auto-indent config model and defaults (depends on: 2)
  - [x] **1.1** Add `AutoIndentMode` to `src/config.rs` with `off` as the default and `neighbor` as the initial enabled mode.
  - [x] **1.2** Thread the new config field through `Config`, `PartialConfig`, and config resolution.
  - [x] **1.3** Validate invalid auto-indent values during startup loading so bad config fails early.
  - [x] **1.4** Update `docs/config.md` to document the new enum-style setting and its default.

- [x] **2.** Add buffer indentation inference helpers (depends on: none)
  - [x] **2.1** Add a focused helper that inspects nearby non-blank lines and returns the exact leading-whitespace prefix for a new line.
  - [x] **2.2** Encode the local selection rule so the more-indented relevant neighbor wins when both sides are usable.
  - [x] **2.3** Keep the helper read-only and reusable from insert-mode and open-line paths.

- [x] **3.** Update insert-mode newline handling (depends on: 1, 2)
  - [x] **3.1** Route `<Enter>` through the new auto-indent mode so `off` preserves the current plain newline behavior.
  - [x] **3.2** When auto-indent is enabled, insert newline plus inferred indentation as a single edit.
  - [x] **3.3** Keep repeat capture aligned with the exact inserted text so dot-repeat replays the same newline-and-indent sequence.

- [x] **4.** Update open-line commands to reuse the same indentation behavior (depends on: 1, 2)
  - [x] **4.1** Apply auto-indent when `o` and `O` create editable lines.
  - [x] **4.2** Preserve existing count behavior and cursor placement after the new line is inserted.
  - [x] **4.3** Keep the disabled `off` path equivalent to the current open-line behavior.

## Testing
- [x] **5.** Add regression tests for auto-indent config and inference (depends on: 1, 2)
  - [x] **5.1** Verify `off` is the default and `neighbor` parses from config.
  - [x] **5.2** Verify invalid config values are rejected.
  - [x] **5.3** Verify inference ignores blank lines and chooses the more-indented relevant neighbor.

- [x] **6.** Add editor behavior tests for newline creation and open-line commands (depends on: 3, 4)
  - [x] **6.1** Verify `<Enter>` inserts the expected indentation when auto-indent is enabled.
  - [x] **6.2** Verify `<Enter>` remains plain newline insertion when auto-indent is `off`.
  - [x] **6.3** Verify `o` and `O` create indented blank lines consistent with insert-mode behavior.
  - [x] **6.4** Verify ordinary character insertion is unchanged.

- [x] **7.** Run the project checks and fix any regressions introduced by the feature (depends on: 1, 2, 3, 4, 5, 6)
  - [x] **7.1** Run `cargo check` to confirm the build is clean.
  - [x] **7.2** Run the relevant unit and integration tests for config, buffer, editor, and open-line behavior.

## Completion Summary

| Area | Tasks | Done | Status |
| --- | --- | ---: | --- |
| Config | 1 | 1 | Done |
| Buffer inference | 1 | 1 | Done |
| Insert mode | 1 | 1 | Done |
| Open-line behavior | 1 | 1 | Done |
| Testing | 3 | 3 | Done |
| Total | 7 | 7 | Done |
