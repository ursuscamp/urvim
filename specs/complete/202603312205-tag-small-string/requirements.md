# Tag Small String

## Summary

Replace the internal `String` storage used by `Tag` with a small-string type so cloned tags are cheap while preserving the existing public behavior of validated syntax tags.

## Problem Statement

`Tag` is used throughout syntax loading, theme resolution, and tests. It is currently a thin wrapper around `String`, so every clone of a tag allocates and copies heap-backed text. Because tags are short, validated identifiers that are frequently duplicated, a small-string representation should reduce allocation overhead and make cloning inexpensive without changing how callers use tags.

## User Stories

- As a developer, I want `Tag` clones to be cheap, so that theme and syntax code can pass tags around without unnecessary allocations.
- As a developer, I want the public `Tag` API to continue working, so that existing code can keep parsing, comparing, storing, and displaying tags the same way.
- As a maintainer, I want the new storage type to fit the existing validation rules, so that invalid tag inputs are still rejected consistently.

## Functional Requirements

- [ ] **REQ-001**: `Tag` must continue to represent a validated hierarchical syntax tag.
- [ ] **REQ-002**: `Tag::parse` must continue to accept the same valid inputs and reject the same invalid inputs as before.
- [ ] **REQ-003**: `Tag` cloning must become inexpensive enough that callers can duplicate tags freely without relying on heap-backed `String` copies.
- [ ] **REQ-004**: The public `Tag` API must continue to support string access, display formatting, ordering, hashing, and equality comparisons.
- [ ] **REQ-005**: Existing call sites that store `Tag` values in maps, sets, spans, or theme structures must continue to compile and behave the same after the change.
- [ ] **REQ-006**: The chosen small-string type must preserve the current ownership semantics of `Tag`, including cheap cloning and stable string slices through `as_str`.
- [ ] **REQ-007**: Invalid tag text must still be rejected before storage, including empty strings, uppercase segments, malformed separators, and non-conforming segment characters.
- [ ] **REQ-008**: The change must not alter tag ordering or parent-chain behavior used by syntax style lookup.

## Non-Functional Requirements

- **Performance**: Tag cloning should avoid heap allocation for common short tag values.
- **Compatibility**: The change must work with the existing Rust edition and project dependency set.
- **Reliability**: Tag validation and equality semantics must remain deterministic and stable.
- **Usability**: The public-facing `Tag` API should remain straightforward for callers that currently treat it as an owned string-like type.

## Acceptance Criteria

- [ ] **AC-001**: The project builds successfully with the updated `Tag` representation.
- [ ] **AC-002**: Existing `Tag` tests continue to pass without changing the observable parse rules.
- [ ] **AC-003**: `Tag` values can be cloned repeatedly without introducing visible API changes at call sites.
- [ ] **AC-004**: Theme and syntax code that stores `Tag` values in collections continues to work with no behavioral regressions.
- [ ] **AC-005**: Invalid tag inputs are still rejected with the same validation behavior as before.

## Out of Scope

- Changing tag syntax rules or the hierarchical tag format.
- Redesigning theme resolution or syntax loading behavior beyond the internal `Tag` storage change.
- Replacing any unrelated string-like types elsewhere in the codebase.

## Assumptions

- `Tag` should remain a small owned value that can be cloned cheaply.
- The best small-string crate will be chosen during design based on fit with the current API, clone behavior, and dependency footprint.
- Existing consumers of `Tag` should not need to change how they call `parse`, `as_str`, or `Display`.

## Dependencies

- A small-string crate that supports cheap cloning or copy-like behavior for short strings.
- The current `Tag` validation and parent-chain logic in `src/theme/tag.rs`.
- Existing syntax and theme call sites that rely on `Tag` remaining hashable, comparable, and displayable.
