# Key String Trie Inserts - Implementation Tasks

## Overview
Add a canonical key-string parser and a `TrieKeymap::insert_str` helper, migrate literal keymap registrations to the new helper, and add tests that prove the string form matches the existing sequence form.

## Backend
- [x] **1.** Add canonical key-string parsing to trie keymap registration.
  - [x] **1.1** Implement a tokenizer that converts canonical strings into `Vec<String>` key tokens.
  - [x] **1.2** Add `TrieKeymap::insert_str` as a wrapper around the existing insertion path.
  - [x] **1.3** Keep `insert` and `insert_sequence` available for existing call sites.
  - [x] **1.4** Add doc comments for the new helper and parser behavior.

- [x] **2.** Migrate literal keymap construction to the string helper.
  - [x] **2.1** Replace single-key insertions in insert mode with `insert_str`.
  - [x] **2.2** Replace literal single- and multi-key insertions in normal mode with `insert_str`.
  - [x] **2.3** Update any literal trie bindings in tests so they use the string helper when it improves readability.
  - [x] **2.4** Leave dynamic or programmatically assembled sequences on `insert_sequence` when that remains clearer.

## Testing
- [x] **3.** Add parser and helper coverage.
  - [x] **3.1** Test single-key, multi-key, and mixed canonical strings.
  - [x] **3.2** Test bracketed special tokens such as `<Esc>`, `<C-s>`, `<LessThan>`, and `<GreaterThan>`.
  - [x] **3.3** Test that malformed canonical strings are rejected or otherwise not treated as valid bindings.

- [x] **4.** Verify the workspace still builds cleanly.
  - [x] **4.1** Run `cargo test`.
  - [x] **4.2** Run `cargo check`.

## Completion Summary

| Area | Total | Done |
|------|-------|------|
| Backend | 2 | 2 |
| Testing | 2 | 2 |
| Total | 4 | 4 |
