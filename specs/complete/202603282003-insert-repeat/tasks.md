# Insert-Mode Dot Repeat - Implementation Tasks

## Overview

Extend dot-repeat so that completed change commands like `cw` replay both the structural change and the inserted text. The implementation should keep the existing normal-mode repeat entry point, add a committed text payload to repeat state, and cover the flow with focused tests.

## Editor and Repeat State

- [x] **1.** Extend dot-repeat state to store committed insert text `(depends on: none)`
  - [x] **1.1** Add an `insert_text` payload to the repeat record and document its constraints.
  - [x] **1.2** Keep the repeat record compatible with existing count handling and non-insert repeat sources.
  - [x] **1.3** Ensure repeat state is only updated after a fully successful edit, not after an abandoned insert session.

- [x] **2.** Add insert-session capture and commit plumbing `(depends on: 1)`
  - [x] **2.1** Add a way for insert mode to accumulate the committed text entered during a session.
  - [x] **2.2** Expose the committed insert text when insert mode exits successfully.
  - [x] **2.3** Clear transient insert capture state when insert mode is abandoned or finalized.

## Replay Pipeline

- [x] **3.** Replay inserted text after the structural dot-repeat action `(depends on: 1, 2)`
  - [x] **3.1** Update repeat resolution so `.` can replay the stored action plus stored insert text as one completed edit.
  - [x] **3.2** Apply the stored text through the existing buffer text insertion path instead of re-entering insert mode.
  - [x] **3.3** Preserve existing count override behavior for `2.` and similar repeat commands.
  - [x] **3.4** Keep the stored repeat record intact if replay fails partway through.

- [x] **4.** Add a reusable buffer text insertion helper if needed `(depends on: 2, 3)`
  - [x] **4.1** Reuse the existing Unicode-aware insertion behavior for multi-character replay payloads.
  - [x] **4.2** Ensure the replay helper updates the cursor consistently with normal insert behavior.
  - [x] **4.3** Keep the helper focused so it can be used by both insert mode and dot repeat.

## Testing and Documentation

- [x] **5.** Add editor-level tests for repeat state and insert commit behavior `(depends on: 1, 2)`
  - [x] **5.1** Verify change commands that enter insert mode can store committed insert text for repeat.
  - [x] **5.2** Verify abandoned insert sessions do not replace the last valid repeat record.
  - [x] **5.3** Verify non-insert repeat sources still behave as before.

- [x] **6.** Add integration tests for full dot-repeat replay `(depends on: 3, 4)`
  - [x] **6.1** Verify `cw`, typed text, and `<Esc>` can be repeated with `.` at another cursor location.
  - [x] **6.2** Verify a count before `.` overrides the stored repeat count for the full replay.
  - [x] **6.3** Verify the repeated insertion preserves the exact committed text, including multi-character and multiline input.
  - [x] **6.4** Verify existing non-insert dot-repeat behavior still works after the change.

- [x] **7.** Update public docs comments and run verification `(depends on: 1, 2, 3, 4, 5, 6)`
  - [x] **7.1** Add or update public doc comments for any new exported repeat-state or insert-capture APIs.
  - [x] **7.2** Run `cargo check` and the relevant test suite to confirm the feature and catch clippy-adjacent issues early.

## Completion Summary

| Area | Status |
|---|---|
| Editor and Repeat State | Done |
| Replay Pipeline | Done |
| Testing and Documentation | Done |
| Total | 7 / 7 tasks complete |
