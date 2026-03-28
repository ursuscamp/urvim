# Save Indicators and Save-on-Command - Implementation Tasks
## Overview
Total: 5 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [ ] **1.** Add modified-state tracking and save-baseline handling to the buffer layer
  - [ ] **1.1** Store the last saved buffer snapshot and expose a read-only modified-state API.
  - [ ] **1.2** Update save/load/path assignment flows so the saved baseline is refreshed at the right times.
  - [ ] **1.3** Ensure undo and redo clear the modified marker when buffer contents match the saved baseline.
  - [ ] **1.4** Add buffer-layer tests for modified-state transitions, save-baseline resets, and no-op named/unnamed behavior.

- [ ] **2.** Add the save action and bind `<C-s>` in both editor modes
  - [ ] **2.1** Add `SaveBuffer(Option<BufferId>)` to the editor action enum and route it through the main event loop.
  - [ ] **2.2** Bind `<C-s>` in normal mode and insert mode as `SaveBuffer(None)` without changing the current mode after a successful save.
  - [ ] **2.3** Make unnamed-buffer saves a no-op instead of inventing a filename.
  - [ ] **2.4** Add action and keybinding tests for the save shortcut.

- [ ] **3.** Render modified indicators in the tab bar and status bar
  - [ ] **3.1** Extend the active-buffer status context with modified-state information.
  - [ ] **3.2** Update tab labels to include a compact modified marker for dirty buffers.
  - [ ] **3.3** Add a themed UI style slot for modified markers to the theme model and schema.
  - [ ] **3.4** Apply the themed modified-marker style in the tab bar and status bar.
  - [ ] **3.5** Update the footer text format so the modified marker appears alongside the existing metadata.
  - [ ] **3.6** Add rendering tests for modified and clean buffers in both UI regions.

- [ ] **4.** Update built-in themes for the modified marker style
  - [ ] **4.1** Define the modified-marker style in each built-in theme file.
  - [ ] **4.2** Add or update tests to confirm built-in themes load the new style slot.

- [ ] **5.** Move filetype refreshes to save-time only
  - [ ] **5.1** Remove filetype refresh calls from edit-time mutation paths.
  - [ ] **5.2** Refresh filetype after successful saves and when loading or assigning a path.
  - [ ] **5.3** Add tests covering shebang changes that update filetype only after save.

- [ ] **6.** Verify the feature end to end
  - [ ] **6.1** Run `cargo check` and fix compile errors or warnings.
  - [ ] **6.2** Run targeted buffer, tab bar, status bar, and input tests.
  - [ ] **6.3** Run the full test suite before marking the work complete.

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 5 | 0 | 0% |
| Testing | 1 | 0 | 0% |
| **Total** | **6** | **0** | **0%** |
