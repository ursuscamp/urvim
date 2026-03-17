# Menu Key Support - Implementation Tasks

## Overview

Total: 6 tasks
Add Menu key support to the keyboard handling system.

## Implementation

### keys.rs Changes
- [x] **1.** Add `Menu` variant to `KeyCode` enum (test: code compiles)
- [x] **2.** Add Menu to `special_name()` method (test: returns "Menu")

### escape.rs Changes
- [x] **3.** Add CSI 29~ handling in `try_parse_csi_tilde()` (test: parse "\x1b[29~" as Menu)

### Testing
- [x] **4.** Add unit test for Menu key parsing (test: test passes)
- [x] **5.** Add unit test for Menu key with modifiers (test: test passes)
- [x] **6.** Verify canonical string format (test: returns "<Menu>")

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| keys.rs | 2 | 2 | 100% |
| escape.rs | 1 | 1 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **6** | **6** | **100%** |
