# Overlay Components as Widgets - Implementation Tasks

## Overview
Refactor command-line and notification overlays into dedicated widgets, update layout orchestration to host widget-based overlays, preserve behavior parity, and validate via regression testing.

## Widget Extraction
- [x] **1.** Create command-line widget module.
  - [x] **1.1** Move command-line state and input handling into widget-owned state.
  - [x] **1.2** Implement `Widget` trait event handling for key/paste interactions.
  - [x] **1.3** Implement centered floating render via shared floating frame utility.

- [x] **2.** Create notification banner widget module.
  - [x] **2.1** Move banner render logic behind widget render hook.
  - [x] **2.2** Implement tick handling for prune/advance behavior via widget event handling.
  - [x] **2.3** Implement top-right floating render via shared floating frame utility.

## Layout Integration
- [x] **3.** Integrate overlay widgets into layout orchestration.
  - [x] **3.1** Add widget instances to layout state.
  - [x] **3.2** Route overlay-first UI events through widgets and merge emitted intents.
  - [x] **3.3** Delegate overlay rendering to widget render calls.
  - [x] **3.4** Preserve command-line cursor override for terminal cursor placement.

- [x] **4.** Remove direct layout-owned overlay internals.
  - [x] **4.1** Remove redundant command-line render/event logic from layout.
  - [x] **4.2** Remove notification render shortcuts no longer needed outside widget path.
  - [x] **4.3** Keep command parse/execute and notification queue logic focused in appropriate modules.

## Behavior Parity and Tests
- [x] **5.** Add/adjust tests for command-line widget behavior.
  - [x] **5.1** Input edit/history/submit/cancel parity tests.
  - [x] **5.2** Overlay close-after-submit parity tests.
  - [x] **5.3** Command execution success/error notification parity tests.

- [x] **6.** Add/adjust tests for notification widget behavior.
  - [x] **6.1** Queue progression and TTL parity tests.
  - [x] **6.2** Placement/wrapping render parity tests.
  - [x] **6.3** Tick routing and redraw behavior parity tests.

## Documentation and Governance
- [x] **7.** Update architecture documentation and contributor guidance.
  - [x] **7.1** Document widget-based overlay architecture and responsibilities.
  - [x] **7.2** Confirm project guidance explicitly states contained UI components should be widgets.

## Build, Lint, and Validation
- [x] **8.** Run formatting and verification.
  - [x] **8.1** Run `cargo fmt`.
  - [x] **8.2** Run `cargo check` and resolve warnings.
  - [x] **8.3** Run targeted and relevant regression tests.

## Completion Summary
| Metric | Value |
|---|---:|
| Total Tasks | 8 |
| Completed | 8 |
| Remaining | 0 |
| Progress | 100% |
