# Command Line for Save and Edit - Implementation Tasks

## Overview
Implement a normal-mode `:` command-line overlay backed by a shared floating window abstraction, add `save`/`edit` command parsing and execution, refactor notification banner rendering to use the shared floating primitive, and add regression/unit tests.

## Core UI and Architecture
- [x] **1.** Introduce reusable floating window abstraction for bordered overlays.
  - [x] **1.1** Identify current notification-banner-specific floating/border layout code and extract shared geometry + frame rendering responsibilities.
  - [x] **1.2** Add a generic floating window spec/model supporting anchor policies (`Center`, `TopRight`) and bounded sizing.
  - [x] **1.3** Add shared render entrypoint that draws border and exposes an inner content rect for caller rendering.
  - [x] **1.4** Preserve existing theme-driven border styling and glyph behavior through the new abstraction.

- [x] **2.** Refactor notification banner to use shared floating window abstraction.
  - [x] **2.1** Replace notification-specific floating frame drawing with shared floating renderer calls.
  - [x] **2.2** Verify existing notification placement, wrapping, and TTL behavior remains unchanged.
  - [x] **2.3** Add/update regression tests for notification banner rendering behavior impacted by refactor.

- [x] **3.** Add command-line overlay widget and lifecycle plumbing.
  - [x] **3.1** Add command-line UI state model (input buffer, cursor index, session history, history cursor).
  - [x] **3.2** Add root/UI dispatch command(s) to open/close command-line overlay and route input while active.
  - [x] **3.3** Bind `:` in normal mode to open command-line overlay.
  - [x] **3.4** Render command-line overlay in centered bordered floating window using shared abstraction.
  - [x] **3.5** Ensure command-line overlay closes on `Esc` and after any submit attempt.

## Command Parsing and Execution
- [x] **4.** Implement command parser for `save` and `edit` with quoted argument support.
  - [x] **4.1** Add parse model (`ParsedCommand`) covering `save`/`edit` with optional path.
  - [x] **4.2** Implement tokenization/parsing handling quoted paths and malformed quote errors.
  - [x] **4.3** Reject unknown command names and invalid argument arity with explicit parse errors.
  - [x] **4.4** Add parser unit tests for valid, invalid, and edge-case command strings.

- [x] **5.** Implement command execution path and editor integration.
  - [x] **5.1** Implement `save` (no path): save active path-backed buffer, error for unnamed buffer.
  - [x] **5.2** Implement `save <path>`: save-as only when target does not exist; error if path exists.
  - [x] **5.3** Implement `edit` (no path): open new unnamed buffer and switch active view.
  - [x] **5.4** Implement `edit <path>`: switch to existing open buffer by resolved path or open new file-backed buffer.
  - [x] **5.5** Ensure all parse/execute failures publish errors through existing notification system.

## Command-Line Input UX
- [x] **6.** Implement command-line editing and history navigation controls.
  - [x] **6.1** Handle printable text insertion and backspace deletion in command-line input.
  - [x] **6.2** Handle submit via `Enter` and cancel via `Esc`.
  - [x] **6.3** Implement session-only history storage and update policy on submit attempts.
  - [x] **6.4** Implement history navigation bindings for Up/Down and `Ctrl-p`/`Ctrl-n`.
  - [x] **6.5** Ensure history navigation and cursor/input behavior remain stable after cancel/submit cycles.

## Documentation and Tests
- [x] **7.** Add/expand tests for command-line behavior and command execution.
  - [x] **7.1** Add unit tests for command-line key handling (insert/backspace, history nav, submit/cancel).
  - [x] **7.2** Add integration/regression tests for `save` and `edit` command outcomes, including error notifications.
  - [x] **7.3** Add tests validating overlay close-after-submit behavior for success and failure.

- [x] **8.** Update project documentation for new command mode behavior.
  - [x] **8.1** Update user docs describing `:` command line and initial commands (`save`, `edit`).
  - [x] **8.2** Update any relevant UI/interaction docs to mention command-line overlay and history bindings.

## Build, Lint, and Validation
- [x] **9.** Run formatter and static checks.
  - [x] **9.1** Run `cargo fmt`.
  - [x] **9.2** Run `cargo check` and resolve warnings.
  - [x] **9.3** Run targeted tests for updated modules.

## Completion Summary
| Metric | Value |
|---|---:|
| Total Tasks | 9 |
| Completed | 9 |
| Remaining | 0 |
| Progress | 100% |