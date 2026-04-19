# Case Switching Operators

## Summary
Add Vim-style case switching operators `gu`, `gU`, and `g~` to urvim. These operators should behave consistently with the editor's existing operator model, work across all applicable operator-supported modes and targets including visual selections, and use Unicode-aware casing where Rust supports it.

## Problem Statement
Users can currently delete, change, and yank text through Vim-like operators, but urvim does not yet support the common case-editing family. This leaves a gap for fast text transformation workflows, especially when editing identifiers, prose, or mixed-language content that needs case normalization.

## User Stories
- As a Vim user, I want to lowercase selected text with `gu`, so that I can quickly normalize text without leaving normal mode.
- As a Vim user, I want to uppercase selected text with `gU`, so that I can emphasize or transform identifiers and words consistently.
- As a Vim user, I want to toggle case with `g~`, so that I can invert the case of text when correcting or reshaping content.
- As a user working with Unicode text, I want case switching to follow Rust's Unicode casing behavior where available, so that non-ASCII text is handled sensibly.

## Functional Requirements
- [ ] **REQ-001**: `gu`, `gU`, and `g~` MUST be recognized as operator commands in the same interaction style as existing urvim operators.
- [ ] **REQ-002**: Each operator MUST act on the same classes of targets currently supported by other operators, including the same motion and text-object flows where operator-pending behavior is available.
- [ ] **REQ-003**: Each operator MUST also act on the active visual selection when invoked from visual mode.
- [ ] **REQ-004**: `gu` MUST lowercase the targeted text.
- [ ] **REQ-005**: `gU` MUST uppercase the targeted text.
- [ ] **REQ-006**: `g~` MUST toggle the case of the targeted text.
- [ ] **REQ-007**: Case conversion MUST be applied using Rust's Unicode-aware casing behavior where available.
- [ ] **REQ-008**: Text that has no applicable case mapping MUST remain unchanged rather than causing an error.
- [ ] **REQ-009**: If an operator invocation resolves to an empty or otherwise non-editable target, the editor MUST treat it the same way it treats other no-op operator invocations.
- [ ] **REQ-010**: These operators MUST preserve the editor's existing expectations for repeatability, cancellation, and cursor behavior for operator-driven edits.
- [ ] **REQ-011**: These operators MUST not require new configuration to be usable.

## Non-Functional Requirements
- **Compatibility**: Behavior should match Vim-style expectations for the supported operator family as closely as the existing editor model allows.
- **Usability**: The operators should be discoverable and predictable for users already familiar with Vim.
- **Reliability**: Unicode text should be transformed deterministically according to Rust's standard casing behavior.

## Acceptance Criteria
- [ ] **AC-001**: `gu`, `gU`, and `g~` can be invoked wherever existing operators are supported, without introducing a separate interaction pattern.
- [ ] **AC-002**: Applying `gu`, `gU`, or `g~` to a motion or text object updates only the resolved target text.
- [ ] **AC-003**: Applying `gu`, `gU`, or `g~` to an active visual selection updates that selection and leaves the surrounding buffer unchanged.
- [ ] **AC-004**: `gu` converts targeted ASCII and Unicode-capable text to lowercase.
- [ ] **AC-005**: `gU` converts targeted ASCII and Unicode-capable text to uppercase.
- [ ] **AC-006**: `g~` inverts the case of targeted text where casing exists.
- [ ] **AC-007**: Invoking a case operator on text with no case mapping does not crash and leaves unmapped characters unchanged.
- [ ] **AC-008**: Existing operator workflows that currently cancel or no-op on invalid targets continue to do so for these operators.

## Out of Scope
- Adding new motion families or text objects.
- Changing existing delete, change, or yank semantics.
- Introducing configuration flags for case conversion behavior.
- Reworking the editor's operator architecture beyond what is needed to support these three commands.

## Assumptions
- Existing operator-pending handling is the correct place to integrate `gu`, `gU`, and `g~`.
- Rust's standard Unicode casing behavior is the desired source of truth for non-ASCII transformation.
- The editor should follow the same repeat, cancel, and cursor-placement conventions it already uses for comparable operator edits.

## Dependencies
- The current operator dispatch and operator-target resolution flow.
- The editor's text mutation path for applying transformed ranges.
- Rust standard library casing behavior for Unicode-aware case conversion.
