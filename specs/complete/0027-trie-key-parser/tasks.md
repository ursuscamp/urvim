# Trie-Based Key Parser with Multiplicative Counts - Implementation Tasks

## Overview

Total: 14 tasks
Estimated completion: 3-4 hours
Prerequisites: None (this is a refactoring task)

## Core Implementation

- [x] **1.** Create TrieKeymap implementation
  - [x] **1.1** Create TrieNode struct with BTreeMap children and Option<Action> (test: compile check)
  - [x] **1.2** Implement TrieKeymap::new() constructor (test: instantiate)
  - [x] **1.3** Implement TrieKeymap::insert() for single keys (test: insert and get)
  - [x] **1.4** Implement TrieKeymap::insert_sequence() for multi-key bindings (test: insert "gg" and lookup)
  - [x] **1.5** Implement TrieKeymap::get_action() with O(k) lookup (test: verify exact match)
  - [x] **1.6** Implement TrieKeymap::is_prefix() for partial matches (test: verify prefix detection)
  - [x] **1.7** Implement Default trait for TrieKeymap (test: compile check)

- [x] **2.** Create CountParser implementation
  - [x] **2.1** Implement CountParser::is_count_digit() helper (test: verify 1-9 are digits, 0 is not)
  - [x] **2.2** Implement CountParser::parse() to extract action keys and count (test: all example cases from requirements)
  - [x] **2.3** Handle overflow by capping at MAX_COUNT (test: verify large counts are capped)

- [x] **3.** Implement Keymap trait for TrieKeymap
  - [x] **3.1** Implement get_action() using trie lookup (test: integrate with existing trait)
  - [x] **3.2** Implement is_prefix() using trie traversal (test: verify trait implementation works)

- [x] **4.** Update NormalMode to use TrieKeymap and CountParser
  - [x] **4.1** Replace SimpleKeymap with TrieKeymap in NormalMode struct (test: compile check)
  - [x] **4.2** Update NormalMode::new() to use TrieKeymap (test: keys work as before)
  - [x] **4.3** Modify handle_key() to use CountParser for count extraction (test: basic counts work)
  - [x] **4.4** Only wrap action with Count if count > 1 (test: verify no unnecessary wrapping)

- [x] **5.** Update InsertMode to use TrieKeymap
  - [x] **5.1** Replace SimpleKeymap with TrieKeymap in InsertMode (test: compile check)
  - [x] **5.2** Update InsertMode::new() to use TrieKeymap (test: keys work as before)

## Testing

- [x] **6.** Write unit tests for CountParser
  - [x] **6.1** Test single digit counts: `5j` → count=5 (test: assert count is 5)
  - [x] **6.2** Test multi-digit counts: `55dd` → count=55 (test: assert count is 55)
  - [x] **6.3** Test sub-counts: `d5d` → count=5 (test: assert count is 5)
  - [x] **6.4** Test multiplicative: `2d2d` → count=4 (test: assert count is 4)
  - [x] **6.5** Test zero not count: `0` → count=1 (test: assert count is 1, key is "0")
  - [x] **6.6** Test mixed multi-digit: `12d34d` → count=408 (test: assert count is 408)
  - [x] **6.7** Test triple multiplicative: `5d5d5d` → count=125 (test: assert count is 125)

- [x] **7.** Write unit tests for TrieKeymap
  - [x] **7.1** Test single key insert and get (test: assert action matches)
  - [x] **7.2** Test multi-key sequence insert (test: "gg" lookup works)
  - [x] **7.3** Test is_prefix for partial matches (test: "g" is prefix of "gg")
  - [x] **7.4** Test is_prefix returns false for non-prefix (test: "x" is not prefix of "gg")
  - [x] **7.5** Test action at non-leaf node (test: single key "j" has action)

- [x] **8.** Integration tests
  - [x] **8.1** Test full key sequence with counts end-to-end (test: run editor with test input)
  - [x] **8.2** Test multi-key sequences with counts: `5gg` → line 5 (test: verify correct line)
  - [x] **8.3** Test that existing tests pass (test: cargo test)

## Cleanup

- [x] **9.** Remove SimpleKeymap (optional - can keep for reference)
  - [x] **9.1** Verify no remaining uses of SimpleKeymap (test: grep for SimpleKeymap)
  - [x] **9.2** Delete SimpleKeymap code (test: compile check)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Core Implementation | 5 | 5 | 100% |
| Testing | 3 | 3 | 100% |
| Cleanup | 1 | 1 | 100% |
| **Total** | **14** | **14** | **100%** |
