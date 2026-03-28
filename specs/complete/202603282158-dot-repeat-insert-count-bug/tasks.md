# Dot Repeat Insert Count Bug - Implementation Tasks

## Overview

Fix dot repeat so that counted change commands that enter insert mode do not duplicate the committed insert text. The replay path should keep the structural count semantics for the original change, while applying the inserted text exactly once per completed replay.

## Backend

- [x] **1.** Separate structural count handling from insert-text playback in dot-repeat replay `(depends on: none)`
  - [x] **1.1** Refactor `replay_repeat_action` in `src/main.rs` so the committed insert text is not emitted inside the structural count loop.
  - [x] **1.2** Preserve the current `.` count override semantics while making the replay logic explicit about which count applies to the structural action.
  - [x] **1.3** Keep failed replay behavior unchanged so the last valid repeat record remains available.

- [x] **2.** Adjust repeat state plumbing only if the data model needs an explicit structural count `(depends on: 1)`
  - [x] **2.1** Update `RepeatReplay` or repeat resolution in `src/editor/action.rs` if a separate structural count field is needed.
  - [x] **2.2** Keep the repeat record compatible with existing non-insert repeat sources.
  - [x] **2.3** Leave the repeat state storage in `src/globals.rs` consistent with the chosen replay model.

## Testing

- [x] **3.** Add regression tests for counted change repeat behavior `(depends on: 1, 2)`
  - [x] **3.1** Verify `2cw`, typed text, and `<Esc>` replay with `.` inserts the text once, not twice.
  - [x] **3.2** Verify a count before `.` still repeats the completed edit as expected.
  - [x] **3.3** Verify non-insert repeat sources still replay correctly after the fix.

- [x] **4.** Run verification for the repeat path `(depends on: 1, 2, 3)`
  - [x] **4.1** Run `cargo check`.
  - [x] **4.2** Run the relevant editor and repeat tests.

## Completion Summary

| Area | Status |
|---|---|
| Backend | Done |
| Testing | Done |
| Total | 4 / 4 tasks complete |
