# Insert-Mode Tab Insertion - Implementation Tasks
## Overview
Implement configurable insert-mode tab insertion and tab rendering using the finalized settings:

- `tab_insertion`: `tabs` or `spaces`
- `tab_behavior`: `simple` or `smart`
- `tab_width`: positive integer display width

The work should keep buffer contents unchanged, infer indentation from existing buffer text in smart mode, and render tab characters using the configured width.

## Backend
- [x] **1.** Add the new config fields to the startup schema and resolved config, including validation and defaults (depends on: none)
  - [x] **1.1** Update `src/config.rs` to parse `tab_insertion`, `tab_behavior`, and `tab_width`
  - [x] **1.2** Validate that `tab_insertion` accepts only `tabs` or `spaces`
  - [x] **1.3** Validate that `tab_behavior` accepts only `simple` or `smart`
  - [x] **1.4** Validate that `tab_width` is greater than zero and provide a sensible default
- [x] **2.** Add tab-resolution logic for insert mode (depends on: 1)
  - [x] **2.1** Implement a helper that resolves the active tab style from the current buffer and config
  - [x] **2.2** Implement smart indentation inference using the first clear leading-whitespace style in the buffer
  - [x] **2.3** Make smart mode fall back to `tab_insertion` when no clear indentation style exists
  - [x] **2.4** Wire insert-mode `Tab` handling in `src/editor/insert.rs` to insert tabs or spaces based on the resolved style
- [x] **3.** Add tab-width-aware rendering support for buffer views (depends on: 1)
  - [x] **3.1** Add a shared width helper for tab characters that advances to the next tab stop
  - [x] **3.2** Update buffer rendering and cursor-to-screen mapping to use the configured tab width
  - [x] **3.3** Keep the existing Unicode width behavior unchanged for non-tab characters

## Testing
- [x] **4.** Add regression tests for config parsing and resolution (depends on: 1)
  - [x] **4.1** Cover valid `tab_insertion`, `tab_behavior`, and `tab_width` values
  - [x] **4.2** Cover invalid config values and zero-width rejection
- [x] **5.** Add regression tests for insert-mode tab handling (depends on: 2)
  - [x] **5.1** Verify `simple` mode always uses the configured insertion setting
  - [x] **5.2** Verify `smart` mode follows the first clear indentation style in the buffer
  - [x] **5.3** Verify `smart` mode falls back to `tab_insertion` when no style can be inferred
- [x] **6.** Add regression tests for tab rendering and cursor alignment (depends on: 3)
  - [x] **6.1** Verify tabs occupy the configured number of visual columns
  - [x] **6.2** Verify changing `tab_width` changes rendering without changing buffer contents
  - [x] **6.3** Verify cursor placement remains aligned with rendered tab expansion
- [x] **7.** Run `cargo check` and the relevant targeted test suites after implementation (depends on: 1, 2, 3, 4, 5, 6)

## Docs
- [x] **8.** Update `docs/config.md` to document `tab_insertion`, `tab_behavior`, and `tab_width` (depends on: 1)
- [x] **9.** Update any user-facing behavior notes that mention insert-mode `Tab` handling to match the new configuration terminology (depends on: 2, 3)

## Completion Summary
| Section | Done | Total |
| --- | ---: | ---: |
| Backend | 3 | 3 |
| Testing | 4 | 4 |
| Docs | 2 | 2 |
| Overall | 9 | 9 |
