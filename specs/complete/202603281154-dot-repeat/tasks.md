# Dot Repeat - Implementation Tasks

## Overview

Implement basic `.` repeat for normal-mode buffer modifications by adding a repeat action, storing the last successful repeatable edit, and replaying that edit through the existing action pipeline. This phase includes insert-starting commands as repeat sources, but it does not capture or replay inserted text yet.

## Editor and Action Plumbing

- [x] **1.** Add a repeat action to the editor action model and normal-mode keymap `(depends on: none)`
  - [x] **1.1** Introduce an `Action` variant for dot repeat and make it countable where appropriate.
  - [x] **1.2** Map `.` in `NormalMode::new()` to the new repeat action.
  - [x] **1.3** Update action trait helpers so the repeat action does not become its own repeat source.

- [x] **2.** Add repeat-source classification helpers `(depends on: 1)`
  - [x] **2.1** Add an action helper that identifies which successful actions should update dot-repeat state.
  - [x] **2.2** Include direct buffer modifications and insert-starting actions as repeat sources.
  - [x] **2.3** Exclude non-mutating actions, undo/redo, and the repeat action itself from repeat-source updates.

## Repeat State and Replay

- [x] **3.** Add storage for the last repeatable edit `(depends on: 1, 2)`
  - [x] **3.1** Introduce a small repeat record that stores the repeatable action and the count used for it.
  - [x] **3.2** Store the repeat record in editor-global state alongside other persistent editor state.
  - [x] **3.3** Ensure the repeat record is only replaced after successful repeatable actions.

- [x] **4.** Implement repeat replay in the main action loop `(depends on: 1, 2, 3)`
  - [x] **4.1** Resolve `.` into the stored repeat source before dispatching to the layout.
  - [x] **4.2** Preserve a user-supplied count on `.` as an override for the replayed count.
  - [x] **4.3** Prevent replayed `.` actions from overwriting the stored repeat source.
  - [x] **4.4** Keep the existing undo snapshot flow unchanged for successful replayed edits.

## Testing

- [x] **5.** Add editor-level tests for dot repeat parsing and source classification `(depends on: 1, 2)`
  - [x] **5.1** Verify `.` is parsed in normal mode into the repeat action.
  - [x] **5.2** Verify repeat-action metadata does not mark `.` itself as a repeat source.
  - [x] **5.3** Verify a representative set of existing edit actions are recognized as repeat sources.

- [x] **6.** Add integration tests for replay behavior `(depends on: 3, 4)`
  - [x] **6.1** Verify a supported delete/change action can be repeated at a new cursor location with `.`.
  - [x] **6.2** Verify a count before `.` overrides the stored repeat count.
  - [x] **6.3** Verify a non-mutating action does not replace the last valid repeat source.
  - [x] **6.4** Verify insert-starting actions such as `o`, `O`, and change-style actions remain repeatable as structural edits even though inserted text is not replayed yet.

## Completion Summary

| Area | Status |
|---|---|
| Editor and Action Plumbing | Done |
| Repeat State and Replay | Done |
| Testing | Done |
| Total | 6 / 6 tasks complete |
