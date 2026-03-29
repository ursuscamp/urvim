# BigWord Text Objects

## Summary

Add vim-style BigWord text objects for operator-pending editing. This feature introduces `iW` and `aW` so users can select or delete whitespace-delimited runs with the same compositional editing flow already used by existing word and bracket text objects.

## Problem Statement

urvim currently supports:
- `iw` and `aw` for word-boundary text objects
- Bracket and quote text objects for delimiter-based selections

What is missing is the BigWord text-object pair that matches vim's `W` family semantics. Without `iW` and `aW`, users cannot apply operators to punctuation-heavy tokens or whitespace-delimited runs in the same way they can with normal word text objects.

## User Stories

- As a vim user, I want to delete a whitespace-delimited token with `iW`, so that I can edit punctuation-heavy text more efficiently.
- As a vim user, I want to delete a whitespace-delimited token plus its trailing whitespace with `aW`, so that I can remove a token without leaving awkward spacing behind.
- As a urvim user, I want BigWord text objects to behave consistently with existing operator-pending editing, so that I can reuse the same command patterns I already know.

## Functional Requirements

- [ ] **REQ-001**: `iW` resolves to a BigWord text object that selects the whitespace-delimited run under or adjacent to the cursor.
- [ ] **REQ-002**: `aW` resolves to a BigWord text object that selects the same whitespace-delimited run as `iW` plus any trailing whitespace immediately following it.
- [ ] **REQ-003**: BigWord text objects use the existing `Boundary::BigWord` semantics, meaning a BigWord is any contiguous run of non-whitespace characters.
- [ ] **REQ-004**: BigWord text objects work in operator-pending mode with existing operators such as delete and change without requiring a separate mode or action path.
- [ ] **REQ-005**: Count prefixes apply to BigWord text objects in the same multiplicative way as existing text objects.
- [ ] **REQ-006**: BigWord text objects do not change the behavior of `iw` and `aw`.
- [ ] **REQ-007**: If the cursor is on whitespace between BigWords, `iW` and `aW` resolve to the nearby whitespace-delimited region in a way that keeps the selection contiguous and predictable.

## Non-Functional Requirements

- **Compatibility**: BigWord text objects should match vim's `iW` and `aW` behavior closely enough for muscle-memory use.
- **Usability**: The new commands should be discoverable and documented alongside the other supported text objects.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `d`, `i`, `W` produces a delete operation targeting the BigWord under the cursor.
- [ ] **AC-002**: Pressing `d`, `a`, `W` produces a delete operation targeting the BigWord under the cursor plus trailing whitespace.
- [ ] **AC-003**: `iW` on `foo-bar baz` selects `foo-bar` when the cursor is inside that run.
- [ ] **AC-004**: `aW` on `foo-bar baz` selects `foo-bar ` when the cursor is inside `foo-bar`.
- [ ] **AC-005**: A count prefix such as `2diW` applies to two consecutive BigWords.
- [ ] **AC-006**: Existing `iw` and `aw` behavior remains unchanged after the new commands are added.

## Out of Scope

- Sentence text objects such as `is` and `as`
- Paragraph text objects such as `ip` and `ap`
- Tag text objects such as `it` and `at`
- Any change to the definition of `iw` and `aw`
- Visual mode support for text objects

## Assumptions

- BigWord uses the already-defined `Boundary::BigWord` semantics from the codebase.
- Operator-pending input handling already supports adding more text objects without changing the overall command model.
- Documentation updates will be made in the motions reference once implementation is ready.

## Dependencies

- Existing operator-pending text object infrastructure
- Existing `Boundary::BigWord` and `Boundary::BigWordEnd` definitions
- Existing motion and text-object documentation structure
