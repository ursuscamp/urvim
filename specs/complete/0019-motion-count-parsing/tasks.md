# Motion Count Parsing - Implementation Tasks

## Overview

Total: 6 tasks
Pattern: `[1-9][0-9]*` (1+ digits, standard vim behavior)
Implementation: `Action::Count(usize, Box<Action>)`

## Implementation

- [x] **1.** Add Action::Count variant to editor.rs
  - [x] **1.1** Add `Count(usize, Box<Action>)` variant to Action enum (test: compile check)
  - [x] **1.2** Implement `is_countable()` method on Action - repeatable motions (h,j,k,l,w,b,e,W,B,E) (test: unit test)
  - [x] **1.3** Implement `is_line_action()` method on Action - line actions ($,0,^) (test: unit test)
  - [x] **1.4** Implement `with_count()` helper method on Action (test: unit test)

- [x] **2.** Modify NormalMode to track pending count
  - [x] **2.1** Add pending_count field to NormalMode struct (test: compile check)
  - [x] **2.2** Initialize pending_count in NormalMode::new() (test: compile check)
  - [x] **2.3** Add helper methods: is_count_digit(), is_valid_count(), count_from_buffer() (test: unit tests)

- [x] **3.** Implement count prefix detection in NormalMode::handle_key
  - [x] **3.1** Update handle_key to accumulate digits in buffer (test: compile check)
  - [x] **3.2** Detect when buffer forms valid count prefix and wait for more keys (test: manual test with "5j")
  - [x] **3.3** Return Action::Count when motion key follows valid count (test: manual test with "5j")
  - [x] **3.4** Clear pending_count on Escape and invalid sequences (test: manual test)

- [x] **4.** Modify Window to handle Action::Count
  - [x] **4.1** Add match arm for Action::Count in process_action (test: compile check)
  - [x] **4.2** Repeatable: loop count times, execute motion each time (test: compile check)
  - [x] **4.3** Line action: move to target absolute line, then execute action (test: compile check)

- [x] **5.** Test count parsing functionality
  - [x] **5.1** Add unit tests for is_valid_count() (test: unit tests)
  - [x] **5.2** Add unit tests for count prefix handling in NormalMode (test: unit tests)
  - [x] **5.3** Manual testing: 5j, 10w, 3b, 2k, 2l, 2h, 4e (test: manual)
  - [x] **5.4** Manual testing: bigword counts 3W, 2B, 2E (test: manual)
  - [x] **5.5** Manual testing: line position counts 2$, 2^ (test: manual)
  - [x] **5.6** Manual testing: invalid sequences like 0j, Esc during count (test: manual)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 4 | 4 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **5** | **5** | **100%** |
