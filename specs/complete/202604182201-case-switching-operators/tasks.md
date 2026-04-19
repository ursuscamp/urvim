# Case Switching Operators - Implementation Tasks

## Overview
Implement Vim-style `gu`, `gU`, and `g~` as shared case-transform operations that work from normal-mode operator-pending flow and from active visual selections. Keep the feature aligned with urvim's existing operator conventions, Unicode-aware text handling, and documentation standards.

## Backend
- [x] **1.** Add a case-transform representation to the editor action model and key dispatch path so `gu`, `gU`, and `g~` can be recognized from normal mode without changing the existing operator interaction pattern.
  - [x] **1.1** Extend the action/command model to represent lower, upper, and toggle-case requests. (depends on: 1)
  - [x] **1.2** Update normal-mode key parsing so the `g`-prefixed sequences resolve to the new case actions. (depends on: 1.1)

- [x] **2.** Implement a shared case-transform helper in the window/edit layer that can rewrite a resolved characterwise range using Unicode-aware casing rules.
  - [x] **2.1** Read the resolved target text, transform it in memory, and replace the original text atomically. (depends on: 2)
  - [x] **2.2** Preserve cursor placement and no-op behavior in the same way other operator edits do. (depends on: 2.1)
  - [x] **2.3** Ensure the helper works with multi-character Unicode expansions and unchanged characters. (depends on: 2.1)

- [x] **3.** Wire visual-mode selection handling to the same case-transform path so `gu`, `gU`, and `g~` act directly on the active selection.
  - [x] **3.1** Handle characterwise visual selections with the shared transform helper. (depends on: 2)
  - [x] **3.2** Handle linewise visual selections with the shared transform helper or an equivalent linewise adapter. (depends on: 2)
  - [x] **3.3** Keep exit/cursor behavior consistent with the existing visual edit flows. (depends on: 3.1, 3.2)

## Docs
- [x] **4.** Update `docs/motions.md` to document the new case operators alongside the existing operator and text-object sections.
  - [x] **4.1** Add the supported operator table entries for `gu`, `gU`, and `g~`. (depends on: 4)
  - [x] **4.2** Document the normal-mode and visual-mode usage details, including Unicode-aware casing notes. (depends on: 4.1)

## Testing
- [x] **5.** Add regression coverage for operator-pending and visual-mode case switching, including Unicode edge cases and no-op handling.
  - [x] **5.1** Cover lowercase, uppercase, and toggle behavior on ASCII text. (depends on: 2, 3)
  - [x] **5.2** Cover Unicode casing behavior where Rust expands or preserves characters. (depends on: 2, 3)
  - [x] **5.3** Cover visual-mode selections for both characterwise and linewise cases. (depends on: 3)
  - [x] **5.4** Cover empty or invalid target handling so the editor keeps its existing no-op behavior. (depends on: 2, 3)

## Completion Summary

| Category | Total | Done | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 3 | 3 | 0 |
| Docs | 1 | 1 | 0 |
| Testing | 1 | 1 | 0 |
| Overall | 5 | 5 | 0 |
