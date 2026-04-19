# Visual Mode Text Objects - Implementation Tasks

## Overview
Implement character-wise visual-mode text objects as idempotent retargeting: a visual text object updates the active selection to the resolved object, and repeating the same object on an already-matching selection is a no-op.

## Backend
- [x] **1.** Add a visual-mode text-object action path and dispatch it from the visual key handling flow.
  - [x] **1.1** Extend the editor action model with a dedicated visual text-object action or equivalent payload that can carry a `TextObject` and optional count.
  - [x] **1.2** Register visual-mode bindings for `iw`, `aw`, `iW`, `aW`, bracket objects, and quote objects in character-wise visual mode only.
  - [x] **1.3** Route the new action from visual key parsing into the window/selection update path instead of operator processing.
- [x] **2.** Implement idempotent visual selection updates for resolved text objects.
  - [x] **2.1** Add a buffer-view helper that accepts a resolved `TextObjectRange` and updates the active visual selection.
  - [x] **2.2** Compare the resolved range against the current visual selection and skip the update when they already match.
  - [x] **2.3** Preserve visual mode after a successful visual text-object update.
  - [x] **2.4** Leave the selection unchanged when a visual text-object cannot be resolved.
- [x] **3.** Reuse the existing text-object resolver for visual-mode selection updates. `(depends on: 1.1, 2.1)`
  - [x] **3.1** Resolve visual-mode word, BigWord, bracket, and quote objects through the existing `Buffer::get_operator_target_range_with_count` path.
  - [x] **3.2** Keep count handling consistent with the normal-mode text-object resolver.
  - [x] **3.3** Avoid duplicating delimiter-matching or word-boundary scanning logic in the visual-mode path.

## Testing
- [x] **4.** Add regression tests for visual-mode text-object behavior. `(depends on: 1.2, 2.2)`
  - [x] **4.1** Test that `viw` enters visual mode and selects the inner word under the cursor.
  - [x] **4.2** Test that repeating the same visual text object leaves the selection unchanged.
  - [x] **4.3** Test that a different visual text object retargets the active selection.
  - [x] **4.4** Test that invalid visual text-object input leaves the selection unchanged.
  - [x] **4.5** Verify that normal-mode operator-pending text objects still behave as before.
- [x] **5.** Run `cargo check` and the focused test suite for editor and window selection paths. `(depends on: 1.1, 2.1, 4.1)`

## Documentation
- [x] **6.** Update `docs/motions.md` to describe visual-mode text-object behavior and the idempotent repeat rule.
- [x] **7.** Update any public doc comments touched by the implementation so the new action and helper behavior is documented clearly.

## Completion Summary

| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Backend | 3 | 3 | Done |
| Testing | 2 | 2 | Done |
| Documentation | 2 | 2 | Done |
| Total | 7 | 7 | Done |
