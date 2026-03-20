# BUG-0039: ^ (Caret) motion does not wrap to previous line when cursor is on blank line - Implementation Tasks

## Overview

Total: 5 tasks
Estimated completion: 30 minutes
Bug fix for `cursor_content_start_of_line` function

## Implementation

- [x] **1.** Fix `cursor_content_start_of_line` to handle blank line at col 0
  - [x] **1.1** Understand current code flow (done - see bug report)
  - [x] **1.2** Modify `None` branch at line 1488 to not return early when `cursor.col == 0`
  - [x] **1.3** Continue to wrap logic to find previous line's first non-whitespace
  - [x] **1.4** Verify fix with existing test: `test_cursor_content_start_of_line_empty_line`

## Testing

- [x] **2.** Run existing tests to ensure no regression
  - [x] **2.1** Run `cargo test cursor_content_start_of_line`
  - [x] **2.2** Run full test suite

## Verification

- [ ] **3.** Verify the fix manually
  - [ ] **3.1** Create buffer with blank line between content
  - [ ] **3.2** Place cursor on blank line at col 0
  - [ ] **3.3** Press `^` and verify cursor wraps to previous line's first non-whitespace

## Documentation

- [ ] **4.** Update docs/motions.md if needed
  - [ ] **4.1** Document `^` behavior on blank lines

## Edge Cases to Test

- [ ] **5.** Test edge cases
  - [ ] **5.1** Multiple consecutive blank lines - should skip all to find content
  - [ ] **5.2** First line is blank - should stay at col 0
  - [ ] **5.3** Buffer with only blank lines - should handle gracefully

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 4 | 4 | 100% |
| Testing | 2 | 2 | 100% |
| Verification | 1 | 0 | 0% |
| Documentation | 1 | 0 | 0% |
| Edge Cases | 1 | 0 | 0% |
| **Total** | **9** | **6** | **67%** |
