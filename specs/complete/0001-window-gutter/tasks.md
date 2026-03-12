# Window Gutter - Implementation Tasks

## Overview

Total: 13 tasks
Implementation of gutter feature for urvim window
Prerequisites: None - this is a new feature

## Implementation

- [x] **1.** Create Gutter struct in window module
  - [x] **1.1** Add Gutter struct with fields: start_line, visible_rows, total_buffer_lines, last_buffer_line (test: compile check)
  - [x] **1.2** Implement Gutter::new(start_line, visible_rows, total_buffer_lines) constructor (test: create Gutter instance)
  - [x] **1.3** Implement Gutter::calculate_width() - use mathematical digit counting (no string conversion) (test: verify width calculation)
- [x] **2.** Implement Gutter background rendering
  - [x] **2.1** Render background color for ALL visible_rows (not just content rows) (test: verify background fills gutter area)
  - [x] **2.2** Use default colors: bg=ANSI 236, fg=ANSI 245 (test: verify style in rendered cells)
- [x] **3.** Implement Gutter line number rendering
  - [x] **3.1** Calculate buffer line for each screen row (start_line + screen_row) (test: verify correct line numbers)
  - [x] **3.2** Right-align line numbers with 1 space padding on each side (test: verify alignment)
  - [x] **3.3** Track last_buffer_line - skip rendering if same buffer line repeats (for wrapping) (test: verify blank cells for repeated lines)
- [x] **4.** Integrate Gutter with Window - buffer offset
  - [x] **4.1** Modify Window::render() to create Gutter with start_line, visible_rows, total_lines (test: compile check)
  - [x] **4.2** Call Gutter::render() before buffer content (test: verify gutter appears on left)
  - [x] **4.3** Offset content origin by gutter width (test: verify content starts after gutter)
  -4.4** Reduce content size by gutter width (test [x] **: verify content has fewer columns)
- [x] **5.** Integrate Gutter with Window - cursor offset
  - [x] **5.1** Modify Window::visual_cursor() to account for gutter width (test: verify cursor position shifted right)
- [x] **6.** Write unit tests and verify existing tests
  - [x] **6.1** Test gutter width calculation for various buffer sizes (test: cargo test)
  - [x] **6.2** Test gutter rendering with background (test: cargo test)
  - [x] **6.3** Test wrapping detection (same line number repeated) (test: cargo test)
  - [x] **6.4** Test cursor position with gutter (test: cargo test)
  - [x] **6.5** Run existing Window tests - ensure no regressions (test: cargo test window::tests)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Gutter struct | 3 | 3 | 100% |
| Gutter background | 2 | 2 | 100% |
| Gutter line numbers | 3 | 3 | 100% |
| Window buffer offset | 4 | 4 | 100% |
| Window cursor offset | 1 | 1 | 100% |
| Testing | 5 | 5 | 100% |
| **Total** | **18** | **18** | **100%** |
