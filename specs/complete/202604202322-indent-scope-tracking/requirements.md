# Indent Scope Tracking

## Summary
Add buffer-cached indent scope tracking that is built and invalidated alongside syntax parsing cache updates. The indent scope data should be independent from syntax highlight styling so it can be consumed later by indent-based folding and indentation guides.

## Problem Statement
The editor currently has no reusable model of indentation structure, which blocks future features like indent-based folding and indentation guides. If indentation structure is computed ad hoc later, it risks duplicated line scans and inconsistent invalidation behavior. We need a single source of truth that tracks indent scopes with the same lifecycle as syntax cache rebuilding.

## User Stories
- As a user, I want indentation-based structure to be precomputed, so that future folding and guide features can be added without expensive rework.
- As a developer, I want indent scopes cached per buffer, so that all consumers read consistent data.
- As a developer, I want indent scopes invalidated and rebuilt with syntax cache updates, so that both caches stay in sync after edits.

## Functional Requirements
- [ ] **REQ-001**: The buffer must store an indent scope cache as a first-class cached structure independent of syntax highlight spans.
- [ ] **REQ-002**: Indent scope cache invalidation must occur whenever syntax highlight cache invalidation occurs for a buffer.
- [ ] **REQ-003**: Indent scope cache rebuilding must occur in the same rebuild pass used for syntax parsing/highlight cache rebuilding.
- [ ] **REQ-004**: Indentation comparison for scope boundaries must use normalized visual indent width rather than raw leading-whitespace byte sequences.
- [ ] **REQ-005**: Visual indent normalization must expand tab characters using configured tab width when available, and default to width `4` when unavailable.
- [ ] **REQ-006**: The scope tracker must support nested scopes.
- [ ] **REQ-007**: A scope starts at a line and extends through the next line with the same normalized visual indent width, inclusive.
- [ ] **REQ-008**: If no later matching-indent line exists, the scope may close at end-of-file and still be recorded.
- [ ] **REQ-009**: A recorded scope must contain at least one inside line between start and end.
- [ ] **REQ-010**: A whitespace-only line counts as non-empty content for validating the inside-line requirement; only zero-length lines are considered empty.
- [ ] **REQ-011**: The cache must expose per-scope records including at least `start_line`, `end_line`, and `indent_width`.
- [ ] **REQ-012**: The cache must expose per-line lookup data for containing scope(s) to support downstream folding and guide consumers.

## Non-Functional Requirements
- [ ] **NFR-001**: Indent scope tracking must preserve current syntax highlighting behavior and output.
- [ ] **NFR-002**: Cache rebuild performance must remain acceptable for interactive editing workloads on large buffers.
- [ ] **NFR-003**: The implementation must avoid unsafe Rust.
- [ ] **NFR-004**: Public API surfaces introduced for indent scopes must include rustdoc documentation comments.

## Acceptance Criteria
- [ ] **AC-001**: After an edit that invalidates syntax cache, indent scope cache is also marked invalid and rebuilt in the same syntax rebuild cycle.
- [ ] **AC-002**: Two lines with equal visual indentation but different tab/space prefixes are treated as matching scope boundaries.
- [ ] **AC-003**: Nested indentation structures produce nested scope records with correct inclusive `start_line` and `end_line`.
- [ ] **AC-004**: A scope with no explicit closing line is recorded through EOF when at least one inside line exists.
- [ ] **AC-005**: Whitespace-only inside lines satisfy the non-empty-inside requirement, while strictly empty lines do not.
- [ ] **AC-006**: Consumers can read both aggregate scope records and per-line containing-scope lookup data from buffer cache APIs.
- [ ] **AC-007**: Regression tests cover mixed tab/space visual-indent normalization, nested scopes, EOF-closing scopes, and inside-line validation rules.

## Out of Scope
- User-facing fold commands and UI interactions.
- Rendering indentation guides.
- New syntax highlight color/tag behavior.
- Persisting indent scopes outside in-memory buffer caches.

## Assumptions
- Syntax parsing already has a clear buffer-level invalidation and rebuild boundary that can host indent scope tracking work.
- Buffer lines are accessible in a stable line-oriented form during syntax rebuild.
- Existing configuration surfaces can provide tab width or allow a fallback default.

## Dependencies
- Buffer syntax cache lifecycle and rebuild pipeline.
- Buffer line iteration utilities.
- Configuration access for tab width resolution.
- Test harness coverage for syntax/cache-related regressions.
