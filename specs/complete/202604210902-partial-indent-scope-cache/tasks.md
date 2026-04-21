# Partial Indent Scope Cache - Implementation Tasks

## Overview
Refine indent scope tracking so the cache can preserve a valid prefix, remember open scopes at the scan frontier, and resume scanning from the first invalidated line instead of rebuilding the whole buffer. Keep syntax highlighting behavior unchanged while making viewport-limited scope establishment and suffix invalidation incremental.

## Backend
- [x] **1.** Redesign the indent scope cache to preserve a resumable frontier.
  - [x] **1.1** Update the cache data model to store open-scope frontier state alongside completed scope records.
  - [x] **1.2** Change `IndentScope` to represent open and closed scopes in a way that preserves scan resumption state.
  - [x] **1.3** Track the first unscanned line so the cache can distinguish covered prefix lines from invalid suffix lines.

- [x] **2.** Make cache invalidation preserve the valid prefix instead of dropping all scope state.
  - [x] **2.1** Update `invalidate_from` so it truncates only the invalidated suffix.
  - [x] **2.2** Reconstruct or retain the open-scope frontier needed to continue scanning after the invalidation boundary.
  - [x] **2.3** Keep syntax invalidation behavior unchanged while the indent scope cache becomes partially invalidated.

- [x] **3.** Add incremental scope extension for viewport and background passes.
  - [x] **3.1** Teach the cache to extend through a requested target line without rescanning already-covered lines. `(depends on: 1.1, 1.3)`
  - [x] **3.2** Ensure viewport-limited syntax ensure calls only establish scope state through the visible range. `(depends on: 3.1)`
  - [x] **3.3** Ensure background catch-up can resume from the same frontier and complete the remaining suffix. `(depends on: 3.1)`

- [x] **4.** Update buffer and syntax cache coordination to use the incremental path.
  - [x] **4.1** Route foreground syntax ensures through the shared incremental scope builder.
  - [x] **4.2** Route catch-up job completion through the same frontier-aware cache update path.
  - [x] **4.3** Keep scope cache commits aligned with the syntax cache generation they accompany.

- [x] **5.** Preserve public read APIs and documentation comments.
  - [x] **5.1** Keep the existing buffer-facing lookup methods working against the new cache model.
  - [x] **5.2** Update rustdoc comments for any public types or methods whose semantics change.

## Testing
- [x] **6.** Add unit tests for partial cache state and frontier behavior.
  - [x] **6.1** Verify a cache can represent both open and closed scopes at the same time.
  - [x] **6.2** Verify invalidation from a line keeps the prefix intact and marks later lines stale.
  - [x] **6.3** Verify incremental extension resumes from the saved frontier instead of rebuilding the prefix.

- [x] **7.** Add regression tests for viewport-limited establishment and suffix edits.
  - [x] **7.1** Verify a viewport-only ensure establishes scope state only through the viewport end.
  - [x] **7.2** Verify an edit invalidates all scope data after the edited line.
  - [x] **7.3** Verify a later ensure call resumes from `max(file start, invalidated frontier)` and completes the missing suffix.

- [x] **8.** Protect syntax highlighting behavior while changing indent scope internals.
  - [x] **8.1** Verify syntax highlight output remains unchanged before and after partial scope updates.
  - [x] **8.2** Verify syntax cache invalidation and visible highlighting behavior still follow the existing code paths.

- [x] **9.** Run project validation.
  - [x] **9.1** Run `cargo check` and fix any build or warning issues.
  - [x] **9.2** Run the relevant buffer, syntax, and regression tests for the changed paths.

## Completion Summary
| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Backend | 5 | 5 | Done |
| Testing | 4 | 4 | Done |
| Total | 9 | 9 | Done |
