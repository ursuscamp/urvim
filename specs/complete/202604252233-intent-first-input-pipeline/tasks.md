# Intent-First Input Pipeline Refactor - Implementation Tasks

## Overview
Migrate the editor from action-first key completion to intent-first completion, remove UI bridge actions, and preserve behavior through focused regressions and compatibility tests.

## Architecture Migration
- [x] **1.** Introduce intent-first completion contracts in mode handling.
  - [x] **1.1** Update key handling result types to support intent completion.
  - [x] **1.2** Preserve `WaitForMore` and invalid-sequence semantics.
  - [x] **1.3** Adapt mode handlers to emit intent completion values.
  - [x] **1.4** Add `From<Action>` / `From<Command>` implementations and generic `Into` constructors for intent-bearing types.

- [x] **2.** Update keymap payload model for command-capable bindings.
  - [x] **2.1** Refactor keymap storage to carry intent-capable payloads.
  - [x] **2.2** Update trie and character-scan keymap lookup paths.
  - [x] **2.3** Verify count/operator workflows still resolve edit actions correctly.

## Command Path Cleanup
- [x] **3.** Move UI orchestration actions to command intent paths.
  - [x] **3.1** Add explicit command variants for opening command-line overlay, pane/split layout operations, wrap toggling, and quit behavior.
  - [x] **3.2** Bind normal-mode `:` to command intent output.
  - [x] **3.3** Convert split/pane/wrap/quit UI bridge action variants to commands and remove the old action kinds.
  - [x] **3.4** Remove UI bridge action variant(s) for command-line open.

- [x] **4.** Update dispatch pipeline to consume emitted intents directly.
  - [x] **4.1** Refactor main loop key handling branch to process intent completion values.
  - [x] **4.2** Keep existing repeat/snapshot/mode-transition behavior intact for action intents.
  - [x] **4.3** Ensure command intents run through layout command dispatch only.

## Compatibility and Regression
- [x] **5.** Add/adjust tests for intent-first emission and routing.
  - [x] **5.1** Add tests proving `:` emits command intent and opens overlay.
  - [x] **5.2** Add tests proving editing sequences still emit action intents.
  - [x] **5.3** Add tests for overlay-first event precedence with command intents.

- [x] **6.** Run and fix regressions in existing mode/keymap/window/layout tests.
  - [x] **6.1** Update tests dependent on old `HandleKeyResult::Complete(Action)` assumptions.
  - [x] **6.2** Update fixtures/helpers to build expected intent payloads.
  - [x] **6.3** Verify no behavior changes in representative workflows.

## Documentation
- [x] **7.** Update architecture and developer documentation.
  - [x] **7.1** Document intent-first input flow and separation of action vs command responsibilities.
  - [x] **7.2** Document removal of UI bridge actions and migration rationale.

## Build, Lint, and Validation
- [x] **8.** Format, build, and validate.
  - [x] **8.1** Run `cargo fmt`.
  - [x] **8.2** Run `cargo check` and resolve warnings.
  - [x] **8.3** Run focused tests for input/mode/keymap/layout plus full regression sweep as needed.

## Completion Summary
| Metric | Value |
|---|---:|
| Total Tasks | 8 |
| Completed | 8 |
| Remaining | 0 |
| Progress | 100% |