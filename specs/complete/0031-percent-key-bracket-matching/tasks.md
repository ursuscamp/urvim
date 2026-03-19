# Percent Key Bracket Matching - Implementation Tasks

## Overview

Total: 10 tasks
Estimated completion: 1-2 hours
Prerequisites: None - this is a self-contained feature

## Implementation

- [x] **1.** Create bracket matching module
  - [x] **1.1** Create `src/motion/bracket_matcher.rs` module (test: file compiles)
  - [x] **1.2** Implement `find_matching_bracket` function (test: unit tests for basic cases)
  - [x] **1.3** Implement bracket pair mapping (test: verify all 6 pairs)

- [x] **2.** Integrate with normal mode
  - [x] **2.1** Add percent key to normal mode key handler (test: pressing % triggers handler)
  - [x] **2.2** Wire up bracket matcher to key handler (test: verify cursor moves on bracket)

- [x] **3.** Add tests
  - [x] **3.1** Write unit tests for bracket matching (test: all bracket types, nested)
  - [x] **3.2** Test non-bracket characters (test: no movement)
  - [x] **3.3** Test unmatched brackets (test: no movement)

- [x] **4.** Create fixture file for manual testing
  - [x] **4.1** Create `fixtures/bracket_test.txt` with various bracket combinations (test: file exists with content)

- [x] **5.** Update documentation
  - [x] **5.1** Add percent key to `docs/motions.md` (test: documentation exists)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 2 | 2 | 100% |
| Integration | 2 | 2 | 100% |
| Testing | 3 | 3 | 100% |
| Fixture | 1 | 1 | 100% |
| Documentation | 1 | 1 | 100% |
| **Total** | **10** | **10** | **100%** |
