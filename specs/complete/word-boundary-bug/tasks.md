# Word Boundary Bug - Implementation Tasks

## Overview

Total: 5 tasks
This fix addresses the bug where small word motions (w/e/b) skip over non-word characters like "---" instead of treating them as separate words.

## Implementation

- [x] **1.** Fix `next_boundary` for `Boundary::Word` to detect non-word char boundaries
  - [x] **1.1** Modify the word skipping logic in next_boundary (src/buffer.rs:939-967)
  - [x] **1.2** Add detection for non-word, non-whitespace characters as word boundaries (test: "hello---world" should navigate to "---" on first w)
- [x] **2.** Fix `next_boundary` for `Boundary::WordEnd` to handle non-word chars
  - [x] **2.1** Modify the whitespace skipping logic (src/buffer.rs:1034-1043)
  - [x] **2.2** Add detection for end of non-word sequences (test: "hello---world" with e should navigate to end of "---")
- [x] **3.** Add unit tests for non-word boundary scenarios
  - [x] **3.1** Test word forward with non-word chars: "hello---world" (DONE)
  - [x] **3.2** Test word backward with non-word chars (DONE)
  - [x] **3.3** Test word end with non-word chars (DONE)
  - [x] **3.4** Test edge cases: multiple non-word chars, non-word at start/end of line (DONE)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 2 | 2 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **5** | **5** | **100%** |
