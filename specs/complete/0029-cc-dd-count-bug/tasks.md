# BUG-0029: dd and cc count prefix bug - Implementation Tasks

## Overview

Total: 2 tasks
Fix the Count handler for DeleteLine and ChangeLine to delete/change N lines from cursor position instead of going to the Nth line.

## Implementation Tasks

- [x] **1.** Fix Count handler for DeleteLine in Window::process_action
  - [x] **1.1** Modify the Count(DeleteLine) handler to use count as repeat count, not line number (test: cargo check passes)
  - [x] **1.2** Verify "2dd" from line 1 deletes lines 1 and 2 (test: unit test)

- [x] **2.** Fix Count handler for ChangeLine in Window::process_action
  - [x] **2.1** Modify the Count(ChangeLine) handler to use count as repeat count, not line number (test: cargo check passes)
  - [x] **2.2** Verify "2cc" from line 1 changes lines 1 and 2 (test: unit test)

- [x] **3.** Run cargo test to verify all tests pass
  - [x] **3.1** Run cargo test (test: all tests pass)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| DeleteLine Fix | 2 | 2 | 100% |
| ChangeLine Fix | 2 | 2 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **5** | **5** | **100%** |
