# C Motion - Change to End of Line - Implementation Tasks

## Overview

Total: 10 tasks
Estimated completion: 2-3 hours
Dependencies: None (uses existing infrastructure)

## Editor (Action Definition & Keymap)

- [x] **1.** Add `Action::ChangeToLineEnd` variant to Action enum (src/editor.rs)
  - [x] **1.1** Add `ChangeToLineEnd` to `is_countable()` returns true (test: verify countable)
  - [x] **1.2** Add `ChangeToLineEnd` to `resets_remembered_column()` returns true (test: verify resets column)
  - [x] **1.3** Add `ChangeToLineEnd` to `switches_to_insert_mode()` returns true (test: verify mode switch)

- [x] **2.** Register "C" keymap (src/editor.rs)
  - [x] **2.1** Add keymap entry: `"C".to_string()` → `Action::ChangeToLineEnd` (test: press C, verify action triggered)

## Buffer (Core Logic)

- [x] **3.** Implement `Buffer::change_to_line_end(cursor, count)` (src/buffer.rs)
  - [x] **3.1** Handle empty buffer case (test: empty buffer returns cursor at 0,0)
  - [x] **3.2** Handle cursor past end of buffer (test: return None)
  - [x] **3.3** Handle cursor at end of line with count=1 - return same cursor (test: cursor at EOL returns unchanged)
  - [x] **3.4** Clamp count to available lines (test: 10C on 3-line buffer clamps to 3)
  - [x] **3.5** Use `remove(start, end)` to delete from cursor to end of N lines (test: verify deletion)
  - [x] **3.6** Return new cursor at start position (which is now at end of truncated text)

## Window (Action Processing)

- [x] **4.** Add `handle_count_change_to_line_end(count)` method (src/window.rs)
  - [x] **4.1** Get current cursor (test: verify gets correct cursor)
  - [x] **4.2** Call `buffer.change_to_line_end(cursor, count)` (test: verify buffer modified)
  - [x] **4.3** Update cursor position if returned (test: verify cursor updated)
  - [x] **4.4** Return `ActionResult::Handled` (test: verify returns handled)

- [x] **5.** Add handler in `process_action` match (src/window.rs)
  - [x] **5.1** Add `Action::ChangeToLineEnd` match arm (test: press C, action handled)
  - [x] **5.2** Add `Action::Count(count, inner)` for ChangeToLineEnd (test: press 2C, handles count)

## Testing

- [x] **6.** Unit tests for `Buffer::change_to_line_end`
  - [x] **6.1** Test "hello world" with cursor after "hello" → "hello" (test: verify truncation)
  - [x] **6.2** Test cursor at position 0 → empty line (test: verify full deletion)
  - [x] **6.3** Test "hello world" cursor at EOL → unchanged, same cursor (test: verify no-op)
  - [x] **6.4** Test 2C with ["hello world", "second"] → ["hello"] (test: verify multi-line)
  - [x] **6.5** Test count exceeds buffer → clamps correctly (test: verify clamping)

- [x] **7.** Unit tests for Action trait methods
  - [x] **7.1** `is_countable()` returns true (test: assert)
  - [x] **7.2** `resets_remembered_column()` returns true (test: assert)
  - [x] **7.3** `switches_to_insert_mode()` returns true (test: assert)
  - [x] **7.4** `with_count(5)` wraps correctly (test: assert inner is ChangeToLineEnd)

- [x] **8.** Integration test for complete C motion
  - [x] **8.1** Test "hell|o world" with 1C → "hell" in insert mode (test: verify final state)
  - [x] **8.2** Test "hello world" at pos 0 with 1C → "" in insert mode (test: verify full line change)
  - [x] **8.3** Test "hello| world" with 2C → "hello" (test: verify count works)

- [x] **9.** Test C at end of line (no-op but mode switch)
  - [x] **9.1** "hello|" with C → "hello" unchanged, mode switches to insert (test: verify mode switch)

- [x] **10.** Run `cargo check` and fix any warnings/lints
  - [x] **10.1** Run `cargo check` (test: no warnings)
  - [x] **10.2** Fix clippy lints if any (test: clippy passes)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Editor | 2 | 2 | 100% |
| Buffer | 1 | 1 | 100% |
| Window | 2 | 2 | 100% |
| Testing | 5 | 5 | 100% |
| **Total** | **10** | **10** | **100%** |
