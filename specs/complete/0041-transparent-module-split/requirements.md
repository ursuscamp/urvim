# Transparent Module Split

## Summary

Review the largest urvim modules and refactor them into smaller sub-modules while preserving the existing external behavior, public API surface, and editor semantics. The goal is to improve maintainability, navigability, and testability without changing how users interact with the editor.

## Problem Statement

Several core source files have grown into large, multi-responsibility modules that mix state, behavior, parsing, rendering, editing logic, and tests in a single file. In particular, `src/buffer.rs`, `src/window.rs`, `src/editor.rs`, and `src/terminal/mod.rs` are now large enough that it is difficult to reason about ownership boundaries, locate related logic, and make changes with confidence.

This matters because:
- Large modules hide natural responsibility boundaries.
- Internal APIs become implicit instead of deliberate.
- Small changes require reading unrelated code paths.
- Test coverage is harder to organize around focused units.

If this is not addressed, future feature work and bug fixes will continue to accumulate inside already dense files, increasing regression risk and slowing development.

## User Stories

- As a developer, I want large modules split into focused sub-modules so that related logic is easier to find and modify.
- As a developer, I want the refactor to preserve urvim's existing behavior so that internal cleanup does not create user-facing regressions.
- As a developer, I want a staged refactor plan so that the work can be implemented safely in reviewable increments.

## Functional Requirements

- [ ] **REQ-001**: Audit the current module layout and identify the largest multi-responsibility modules that should be split first.
- [ ] **REQ-002**: Define a transparent refactor plan for `src/buffer.rs` that preserves the existing public `Buffer`-related API while extracting focused internal sub-modules.
- [ ] **REQ-003**: Define a transparent refactor plan for `src/window.rs` that preserves the existing public `Window`-related API while extracting focused internal sub-modules.
- [ ] **REQ-004**: Define a transparent refactor plan for `src/editor.rs` that preserves the existing public `Action`, mode, and keymap API while extracting focused internal sub-modules.
- [ ] **REQ-005**: Define a transparent refactor plan for `src/terminal/mod.rs` that preserves the existing public terminal API while extracting focused internal sub-modules.
- [ ] **REQ-006**: For each target module, specify candidate sub-module names, their responsibilities, and the items that remain re-exported from the top-level module.
- [ ] **REQ-007**: Identify coupling and sequencing risks for each target module and document mitigations that keep the refactor behaviorally transparent.
- [ ] **REQ-008**: Define an implementation order that starts with the safest extractions and avoids mixing module boundary changes with behavior changes.
- [ ] **REQ-009**: Define how existing unit tests should be preserved, relocated, or expanded to validate that behavior remains unchanged.
- [ ] **REQ-010**: Require `cargo check` and relevant automated tests to pass after each module split stage.

## Non-Functional Requirements

- **Backward compatibility**: Public behavior, command semantics, cursor semantics, and visible editor output must remain unchanged.
- **Maintainability**: Each extracted sub-module should have a single clear responsibility and a discoverable file layout.
- **Reviewability**: The plan should support small, incremental changes rather than a single disruptive rewrite.
- **Testability**: Existing tests should remain runnable, and newly exposed seams should make targeted tests easier to add.
- **Documentation**: Public modules, types, and methods touched by the refactor should continue to have documentation comments.

## Acceptance Criteria

- [ ] **AC-001**: The spec identifies the highest-priority module split targets based on current code organization and size.
- [ ] **AC-002**: The spec defines concrete sub-module boundaries for `buffer`, `window`, `editor`, and `terminal`.
- [ ] **AC-003**: The spec documents which public items remain available from each top-level module after the split.
- [ ] **AC-004**: The spec includes a staged rollout plan that can be implemented without intentional behavior changes.
- [ ] **AC-005**: The spec documents key coupling risks and mitigation strategies for each target module.
- [ ] **AC-006**: The spec defines verification steps, including `cargo check` and targeted test coverage, for every stage.
- [ ] **AC-007**: The plan keeps inline behavior-preserving refactors separate from any optional deeper architectural redesign.

## Out of Scope

- Adding new editor features or changing keybindings.
- Changing buffer, window, terminal, or mode semantics.
- Replacing existing data structures with new ones.
- Performing a broad architecture rewrite across unrelated modules.
- Refactoring small modules unless they are directly affected by the target splits.

## Assumptions

- Existing public behavior in urvim is the source of truth and must be preserved.
- Internal helper visibility such as `pub(super)` can be used to support extracted sub-modules.
- Re-exporting from `mod.rs` files is acceptable when needed to preserve current call sites.
- Inline test modules may be moved into sibling test modules when it improves readability without changing coverage intent.

## Dependencies

- **Internal**: Current module boundaries in `src/buffer.rs`, `src/window.rs`, `src/editor.rs`, and `src/terminal/mod.rs`
- **Internal**: Existing test coverage that validates buffer editing, window behavior, key parsing, terminal I/O, and mode behavior
- **Blocked by**: None
