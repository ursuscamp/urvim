# BUG-002: "E" key at end of line doesn't wrap to next line - Tasks

## Overview

Total: 5 tasks
Fix the BigWordEnd boundary handling in `next_boundary` to properly wrap to the next line when at the end of a line.

## Implementation

- [x] **1.** Analyze the Boundary::BigWordEnd logic in `next_boundary` (src/buffer.rs:1367-1413)
  - [x] **1.1** Identify exactly where the wrap should happen but doesn't (test: trace through "hello\nworld" case)
  - [x] **1.2** Understand why check_col becomes col (no forward movement) (test: debug output)

- [x] **2.** Fix the BigWordEnd handling to wrap to next line
  - [x] **2.1** Add logic to detect when at end of line with no more words (test: cursor at position 4 of "hello")
  - [x] **2.2** Implement wrap to next line with proper whitespace skipping (test: "hello\n  world" case)
  - [x] **2.3** Find end of first word on next line and return position (test: "hello\nworld" -> position 4)

- [x] **3.** Test the fix
  - [x] **3.1** Test basic wrap: "hello\nworld" at end of line 0 -> end of "world" (test: cargo test)
  - [x] **3.2** Test with leading whitespace: "hello\n  world" -> skip spaces, end of "world" (test: cargo test)

- [x] **4.** Add unit tests for the fix
  - [x] **4.1** Add test: BigWordEnd at end of line wraps to next line (test: cargo test)

- [x] **5.** Run full test suite
  - [x] **5.1** Run cargo test to verify no regressions (test: cargo test)
  - [x] **5.2** Run cargo check for warnings (test: cargo check)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 2 | 2 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **5** | **5** | **100%** |
