# Single-Line Input - Implementation Tasks

## Overview
Implement a reusable single-line input widget with shell-style editing defaults, a consumer-supplied display prompt, and per-key override hooks so command-line and picker overlays can share one input core.

## Widget Core
- [ ] **1.** Add the reusable single-line input widget state and constructor.
  - [ ] **1.1** Store editable text, cursor position, and display prompt prefix.
  - [ ] **1.2** Add accessors and setters for text, cursor, and prompt.
  - [ ] **1.3** Add consumer hooks for live text change notifications and key overrides.

- [ ] **2.** Implement shell-style built-in key handling.
  - [ ] **2.1** Handle `Backspace`, `Delete`, `Ctrl-W`, and `Ctrl-U`.
  - [ ] **2.2** Handle `Ctrl-A`/`Home`, `Ctrl-E`/`End`, `Ctrl-B`/`Left`, and `Ctrl-F`/`Right`.
  - [ ] **2.3** Handle `Alt-B`/`Ctrl-Left` and `Alt-F`/`Ctrl-Right` word movement.
  - [ ] **2.4** Ensure unhandled keys insert text normally.
  - [ ] **2.5** Keep `Enter` and `Esc` inert unless overridden.

## Overlay Integration
- [ ] **3.** Wire the widget into command-line and picker overlays.
  - [ ] **3.1** Set the command-line prompt to `:`.
  - [ ] **3.2** Set the picker prompt to `>`.
  - [ ] **3.3** Route overlay-specific submission and cancellation through consumer overrides rather than the widget core.
  - [ ] **3.4** Use the change callback to drive picker query refreshes.

## Testing
- [ ] **4.** Add widget-level tests for editing behavior.
  - [ ] **4.1** Verify normal typing inserts text when no override handles the key.
  - [ ] **4.2** Verify word deletion and line-start deletion work as expected.
  - [ ] **4.3** Verify word-wise and boundary cursor movement work as expected.
  - [ ] **4.4** Verify `Enter` and `Esc` are no-ops by default.
  - [ ] **4.5** Verify prompt text is display-only and not part of the editable buffer.

- [ ] **5.** Add integration tests for command-line and picker usage.
  - [ ] **5.1** Verify the command line renders `:` and still edits text correctly.
  - [ ] **5.2** Verify the picker renders `>` and still edits text correctly.
  - [ ] **5.3** Verify a consumer override can intercept any key and replace default handling.

## Validation
- [ ] **6.** Run `cargo fmt`, `cargo check`, and relevant focused tests.

## Completion Summary
| Area | Total | Done | Remaining |
| --- | ---: | ---: | ---: |
| Widget Core | 10 | 0 | 10 |
| Overlay Integration | 4 | 0 | 4 |
| Testing | 8 | 0 | 8 |
| Validation | 1 | 0 | 1 |
| Total | 23 | 0 | 23 |
