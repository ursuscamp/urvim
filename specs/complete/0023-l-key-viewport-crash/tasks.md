# BUG-0023: L Key Viewport Crash - Implementation Tasks

## Overview

Total: 3 tasks
This is a simple one-line fix in src/window.rs

## Implementation

- [x] **1.** Fix the arithmetic underflow in L key motion
  - [x] **1.1** Modify line 864 in src/window.rs to use saturating_sub (test: verify fix compiles)
  - [x] **1.2** Run cargo check to verify no warnings (test: cargo check passes)
  - [x] **1.3** Test the fix by running urvim with large count (test: manual test or existing tests pass)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 3 | 3 | 100% |
| **Total** | **3** | **3** | **100%** |
