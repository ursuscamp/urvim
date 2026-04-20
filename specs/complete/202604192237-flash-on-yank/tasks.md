# Flash on Yank - Implementation Tasks

## Overview
Implement a transient yank-flash state that reuses the existing visual selection style, activates only for Normal Mode yanks, expires after 200ms, and restarts when another yank happens before the prior flash ends.

## Backend
- [x] **1.** Add transient yank-flash state to the window buffer view.
  - [x] **1.1** Define a flash data model that can represent characterwise and linewise yanked regions.
  - [x] **1.2** Add `BufferView` methods to start, clear, read, and prune the flash state.
  - [x] **1.3** Ensure a new yank replaces any existing flash immediately. (depends on: 1.2)

- [x] **2.** Teach yank handlers to record the flashed region for Normal Mode yanks.
  - [x] **2.1** Update linewise yank handling so `yy`-style actions trigger a flash after the register is stored. (depends on: 1.2)
  - [x] **2.2** Update operator-based yank handling so normal-mode yank operations trigger a flash after the target range is resolved. (depends on: 1.2)
  - [x] **2.3** Keep Visual Mode yank handling unchanged so it does not start a flash. (depends on: 2.1, 2.2)

- [x] **3.** Reuse the Visual Mode selection style when rendering the transient yank flash.
  - [x] **3.1** Apply the existing `ui.selection` highlight style to the flashed region.
  - [x] **3.2** Render characterwise flashes over the exact yanked span.
  - [x] **3.3** Render linewise flashes across the full yanked lines.
  - [x] **3.4** Keep the current mode and cursor style unchanged while the flash is visible.

- [x] **4.** Add expiration handling so the flash disappears after roughly 200ms.
  - [x] **4.1** Prune expired flash state from the active window tree on tick or redraw cycles. (depends on: 1.2)
  - [x] **4.2** Trigger a redraw when the flash expires so the highlight disappears promptly. (depends on: 4.1)

## Frontend
- [x] **5.** Preserve current yank behavior and user flow.
  - [x] **5.1** Confirm that yanks still copy the correct text to registers.
  - [x] **5.2** Confirm that repeated yanks restart the flash rather than queueing multiple flashes.
  - [x] **5.3** Confirm that the feature remains Normal Mode only.

## Testing
- [x] **6.** Add regression tests for yank flashing.
  - [x] **6.1** Test that a normal-mode characterwise yank starts a transient selection highlight. (test: render output includes selection style)
  - [x] **6.2** Test that a normal-mode linewise yank flashes the full lines. (test: render output includes full-line selection style)
  - [x] **6.3** Test that a Visual Mode yank does not create a flash. (test: no transient selection state)
  - [x] **6.4** Test that a second yank interrupts and restarts the existing flash. (test: newer region replaces older region)
  - [x] **6.5** Test that expired flashes are cleared on tick/redraw processing. (test: flash state becomes empty after timeout)

## Completion Summary
| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Backend | 4 | 4 | Done |
| Frontend | 1 | 1 | Done |
| Testing | 1 | 1 | Done |
| Total | 6 | 6 | Done |
