# Action-to-mode switches - Implementation Tasks
## Overview
Remove hardcoded insert-mode switching from `Action::switches_to_insert_mode()` and make `Action.to_mode` the sole source of truth for mode transitions. Consolidate any duplicated actions that differ only by whether they enter insert mode.

## Backend
- [x] **1.** Audit every action that currently enters insert mode and decide whether its `to_mode` should be set at construction time. `(depends on: none)`
  - [x] **1.1** Update normal-mode keymap entries so insert-entering actions carry `to_mode = Some(ModeKind::Insert)` instead of relying on helper inference.
  - [x] **1.2** Consolidate any action pairs that are identical except for insert-mode switching into a single action definition.
- [x] **2.** Remove the `ActionKind`-based fallback from `Action::switches_to_insert_mode()` and keep only metadata-driven behavior. `(depends on: 1)`
  - [x] **2.1** Preserve correct handling for wrapped actions such as `Count` by forwarding `to_mode` through wrappers.
  - [x] **2.2** Verify the event loop still switches modes solely from `dispatch_action.to_mode` and does not depend on action-kind inference.

## Testing
- [x] **3.** Update unit tests to assert insert-mode transitions through `to_mode` instead of hardcoded action kinds. `(depends on: 1, 2)`
  - [x] **3.1** Adjust existing `switches_to_insert_mode()` coverage for change, append, and open-line actions.
  - [x] **3.2** Add regression coverage for counted/wrapped actions to ensure `to_mode` survives wrapping.
- [x] **4.** Run `cargo check` and the relevant editor tests to confirm the refactor is behaviorally equivalent except for the intended consolidation. `(depends on: 2, 3)`

## Completion Summary
| Item | Status |
| --- | --- |
| Backend refactor | Complete |
| Test updates | Complete |
| Verification | Complete |
