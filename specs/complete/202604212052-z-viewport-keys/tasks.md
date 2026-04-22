# Z Viewport Keys - Implementation Tasks

## Overview
Implement normal-mode `zt`, `zz`, and `zb` viewport positioning commands that align the cursor line to the top, center, or bottom of the focused window without moving cursor position, including clamped behavior near file boundaries.

## Backend
- [x] **1.** Add viewport alignment actions for `zt`, `zz`, and `zb` in the action/model layer.
  - [x] **1.1** Introduce action variants (or equivalent command identifiers) for top/center/bottom viewport alignment.
  - [x] **1.2** Ensure action documentation/comments describe no-count semantics and cursor immutability.

- [x] **2.** Extend normal-mode `z` key-sequence handling to resolve `zt`, `zz`, and `zb`.
  - [x] **2.1** Map `zt` to viewport-top action.
  - [x] **2.2** Map `zz` to viewport-center action.
  - [x] **2.3** Map `zb` to viewport-bottom action.
  - [x] **2.4** Preserve existing behavior for unsupported `z` continuations and clear pending state safely.
  - [x] **2.5** Ensure numeric prefixes do not alter `zt`/`zz`/`zb` behavior. (depends on: 2.1, 2.2, 2.3)

- [x] **3.** Implement focused-window viewport alignment execution.
  - [x] **3.1** Add or reuse a helper that computes desired top visible line from cursor line and anchor target (top/center/bottom).
  - [x] **3.2** Clamp computed top line to valid scroll range for current buffer and viewport height.
  - [x] **3.3** Apply scroll origin updates without modifying cursor line/column.
  - [x] **3.4** Route actions so only the focused window viewport is updated.

## Testing
- [x] **4.** Add regression tests for `zt`, `zz`, and `zb` command resolution and execution.
  - [x] **4.1** Test that `zt` aligns cursor line to top row when possible and keeps cursor position unchanged.
  - [x] **4.2** Test that `zz` aligns cursor line to center row when possible and keeps cursor position unchanged.
  - [x] **4.3** Test that `zb` aligns cursor line to bottom row when possible and keeps cursor position unchanged.
  - [x] **4.4** Test clamped behavior near buffer start/end where exact alignment is impossible.
  - [x] **4.5** Test that numeric prefixes do not change `zt`/`zz`/`zb` outcome.
  - [x] **4.6** Test unsupported `z` continuations do not panic and do not alter cursor position unexpectedly.

## Documentation
- [x] **5.** Update motion documentation for new `z` viewport commands.
  - [x] **5.1** Add `zt`, `zz`, and `zb` behavior to `docs/motions.md` with no-count scope noted.

## Validation
- [x] **6.** Run project validation steps.
  - [x] **6.1** Run targeted/new tests for viewport commands.
  - [x] **6.2** Run `cargo fmt`.
  - [x] **6.3** Run `cargo check` and resolve warnings introduced by this change.

## Completion Summary
| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Backend | 3 | 3 | Done |
| Testing | 1 | 1 | Done |
| Documentation | 1 | 1 | Done |
| Validation | 1 | 1 | Done |
| Total | 6 | 6 | Done |
