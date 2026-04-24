# Generic UI Components and Notification Infrastructure - Implementation Tasks

## Overview
Implement a phased UI-architecture refactor that introduces unified intent dispatch and layered component rendering while preserving existing editing behavior. Deliver the first non-window component as a queued notification banner with adaptive TTL behavior and dual-write notification macros.

## Core Architecture
- [x] **1.** Introduce core UI intent and event types
  - [x] **1.1** Add `UiEvent` and `UiEventResult` types for internal widget routing
  - [x] **1.2** Add `Command` type for UI/app orchestration intents
  - [x] **1.3** Add `Intent` envelope with `Intent::Action(Action)` and `Intent::Command(Command)`
  - [x] **1.4** Add unit tests validating intent/event type behavior and invariants

- [x] **2.** Expand `Widget` contract for generic component lifecycle
  - [x] **2.1** Add defaulted widget lifecycle methods for UI-event handling, layout, focus policy, and widget rendering
  - [x] **2.2** Keep `process_action` as a compatibility shim backed by unified intent dispatch semantics
  - [x] **2.3** Update existing widget implementors (`Layout`, `WindowGroup`, `Window`) to compile and behave consistently with expanded trait
  - [x] **2.4** Add tests for compatibility behavior of existing action paths after trait expansion

- [x] **3.** Add root unified intent dispatcher and event adapter
  - [x] **3.1** Add adapter from terminal `Event` to internal `UiEvent`
  - [x] **3.2** Add root queue/loop to consume emitted `Intent` values in order
  - [x] **3.3** Route `Intent::Action` through existing action semantics
  - [x] **3.4** Route `Intent::Command` through root UI/app orchestration handlers
  - [x] **3.5** Add integration tests for key flows to ensure unified dispatch preserves behavior

- [x] **4.** Add layered render/routing support in root layout
  - [x] **4.1** Define base-layer and overlay-layer render order in layout
  - [x] **4.2** Integrate overlay routing precedence for UI events where applicable
  - [x] **4.3** Ensure existing pane/tree rendering and status bar behavior remains correct
  - [x] **4.4** Add regression tests for split rendering, pane focus, and cursor placement with layering enabled

## Notifications
- [x] **5.** Implement notification data model and queue service
  - [x] **5.1** Add `NotificationLevel` and `NotificationMessage` data types
  - [x] **5.2** Add `NotificationState` with `active` + FIFO `pending` queue (unbounded)
  - [x] **5.3** Implement enqueue behavior preserving strict FIFO order
  - [x] **5.4** Implement adaptive TTL policy (3s no backlog, 1s while backlog exists)
  - [x] **5.5** Implement prune/advance behavior on tick/render checks
  - [x] **5.6** Add focused unit tests for queue ordering, TTL selection, and advancement

- [x] **6.** Implement notification banner widget
  - [x] **6.1** Add top-right banner rendering using active queued notification
  - [x] **6.2** Add style resolution for info/warn/error with safe theme fallbacks
  - [x] **6.3** Add clipping/width handling for narrow terminals and long messages
  - [x] **6.4** Ensure banner does not steal editing focus or disrupt cursor behavior
  - [x] **6.5** Add rendering tests for alignment, style-level distinctions, and expiry transitions

- [x] **7.** Implement dual-write notification macros and bridge
  - [x] **7.1** Add `notify!(level, ...)` macro with formatting support
  - [x] **7.2** Add `notify_info!`, `notify_warn!`, and `notify_error!` convenience macros
  - [x] **7.3** Bridge macro calls to both logging and notification enqueue paths
  - [x] **7.4** Ensure enqueue failures degrade gracefully to log-only behavior
  - [x] **7.5** Add unit tests validating macro output routing and level mapping

- [x] **8.** Integrate notification call sites
  - [x] **8.1** Emit info notification on successful file-save operations
  - [x] **8.2** Emit error notifications for curated user-impacting failures (save/open/theme/config/job-visible failures)
  - [x] **8.3** Preserve log-only behavior for intentionally non-user-facing diagnostics
  - [x] **8.4** Add integration tests covering save success and representative error notifications

## Migration and Cleanup
- [x] **9.** Main loop integration and compatibility validation
  - [x] **9.1** Integrate UI-event routing and intent consumption into `main.rs` event loop
  - [x] **9.2** Validate undo/redo, repeat, mode transitions, and paste paths under unified dispatch
  - [x] **9.3** Add regression tests for normal/insert/visual workflows after integration

- [x] **10.** Remove transitional direct `process_action` entrypoints
  - [x] **10.1** Identify remaining direct call sites that bypass unified intent dispatch
  - [x] **10.2** Move remaining call sites to unified intent path (depends on: 3.2)
  - [x] **10.3** Remove transitional compatibility shims once no longer needed
  - [x] **10.4** Add final regression tests confirming no behavior change from shim removal

## Verification
- [x] **11.** Project quality gates
  - [x] **11.1** Add/update Rust unit tests for all new modules
  - [x] **11.2** Run `cargo fmt`
  - [x] **11.3** Run `cargo check`
  - [x] **11.4** Run `cargo clippy --all-targets --all-features`
  - [x] **11.5** Run relevant test subsets plus full `cargo test` as needed

## Completion Summary

| Metric | Count |
|---|---:|
| Total Tasks | 11 |
| Completed Tasks | 11 |
| Remaining Tasks | 0 |
| Progress | 100% |
