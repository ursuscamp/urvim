# Transparent Module Split - Implementation Tasks

## Overview

Total: 17 tasks
This plan splits the largest urvim modules into directory-backed sub-modules while preserving behavior and public APIs.
Key milestones: prepare stable public module shells, split `editor`, split `window`, split `terminal`, split `buffer`, then verify the full codebase.

## Module Preparation

- [ ] **1.** Prepare directory-backed module shells for each target module (test: `cargo check`)
  - [x] **1.1** Replace `src/editor.rs` with `src/editor/mod.rs` while preserving current imports and exports (test: `cargo check`)
  - [x] **1.2** Replace `src/window.rs` with `src/window/mod.rs` while preserving current imports and exports (test: `cargo check`)
  - [x] **1.3** Replace `src/terminal/mod.rs` layout only as needed to host new split files without changing public API (test: `cargo check`)
  - [x] **1.4** Replace `src/buffer.rs` with `src/buffer/mod.rs` while preserving current imports and exports (depends on: 1.1, test: `cargo check`)

## Editor

- [x] **2.** Split `editor` into focused sub-modules (depends on: 1.1, test: targeted editor tests + `cargo check`)
  - [x] **2.1** Extract action definitions and metadata into `src/editor/action.rs` (test: action imports compile)
  - [x] **2.2** Extract shared keymap machinery into `src/editor/keymap.rs` (test: key sequence parsing tests pass)
  - [x] **2.3** Extract count parsing into `src/editor/count.rs` (implemented inside `src/editor/keymap.rs`, test: count parsing tests pass)
  - [x] **2.4** Extract mode traits/shared behavior into `src/editor/mode.rs` (test: mode construction compiles)
  - [x] **2.5** Extract normal-mode bindings into `src/editor/normal.rs` (test: normal mode keybinding tests pass)
  - [x] **2.6** Extract insert-mode bindings into `src/editor/insert.rs` (test: insert mode keybinding tests pass)

## Window

- [x] **3.** Split `window` by responsibility-focused impl blocks (depends on: 1.2, test: window-focused tests + `cargo check`)
  - [x] **3.1** Extract geometry helpers into `src/window/geometry.rs` (test: compile + rendering helpers behave the same)
  - [x] **3.2** Extract viewport and scroll behavior into `src/window/view.rs` (test: viewport tests pass)
  - [x] **3.3** Extract render-model assembly into `src/window/render.rs` (test: rendering tests pass)
  - [x] **3.4** Extract gutter logic into `src/window/gutter.rs` (test: gutter tests pass)
  - [x] **3.5** Extract motion and command helper impls into `src/window/motions.rs` and `src/window/commands.rs` (test: action processing tests pass)

## Terminal

- [x] **4.** Split terminal responsibilities without changing terminal behavior (depends on: 1.3, test: terminal-focused tests + `cargo check`)
  - [x] **4.1** Extract lifecycle code into `src/terminal/lifecycle.rs` (test: terminal setup/teardown tests pass)
  - [x] **4.2** Extract output helpers into `src/terminal/output.rs` (test: terminal write tests pass)
  - [x] **4.3** Extract input polling and paste handling into `src/terminal/input.rs` (test: key and paste parsing tests pass)
  - [x] **4.4** Extract test backend code into `src/terminal/test_backend.rs` (test: terminal test harness still passes)

## Buffer

- [x] **5.** Split `buffer` into the smallest safe set of cohesive internal modules (depends on: 1.4, test: buffer-focused tests + `cargo check`)
  - [x] **5.1** Extract edit operations into `src/buffer/edit.rs` (test: insertion/deletion/join tests pass)
  - [x] **5.2** Extract cursor and positioning helpers into `src/buffer/cursor.rs` (test: cursor movement tests pass)
  - [x] **5.3** Extract boundary traversal into `src/buffer/boundary.rs` (test: word motion tests pass)
  - [x] **5.4** Extract text-object logic into `src/buffer/text_object.rs` (test: text object tests pass)
  - [x] **5.5** Extract undo/redo support into `src/buffer/undo.rs` (test: undo/redo tests pass)
  - [x] **5.6** Extract file I/O and Unicode helpers into focused internal files such as `src/buffer/io.rs`, `src/buffer/search.rs`, and `src/buffer/unicode.rs` when justified by cohesion (test: load/save and grapheme behavior tests pass)

## Verification

- [x] **6.** Reorganize large inline tests into sibling test modules where it improves readability without changing coverage intent (depends on: 2, 3, 4, 5, test: `cargo test`)
- [x] **7.** Run full verification after all module splits (depends on: 6, test: `cargo check` and `cargo test`)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Module Preparation | 1 | 1 | 100% |
| Editor | 1 | 1 | 100% |
| Window | 1 | 1 | 100% |
| Terminal | 1 | 1 | 100% |
| Buffer | 1 | 1 | 100% |
| Verification | 2 | 2 | 100% |
| **Total** | **7** | **7** | **100%** |
