# Background Worker Framework
## Summary
urvim will gain a general job framework that can run internal editor jobs off the main input/render path. The first user-visible use case will be syntax catch-up: the editor should render the visible viewport immediately, then continue highlighting the rest of the file in the background so large files feel responsive when the cursor jumps far ahead.

## Problem Statement
Large syntax-highlighted files can pause when the editor needs to compute highlight state far beyond the currently visible viewport. Today, the syntax path is synchronous, so a long jump to the end of a file can force the editor to spend time tokenizing lines that the user cannot yet see. The project also has other likely future needs for deferred work, so the fix should introduce a reusable worker framework instead of a syntax-only special case.

## User Stories
- As a user opening a large file, I want the current viewport to appear quickly, so that the editor feels responsive even when the file is expensive to highlight.
- As a user jumping deep into a file, I want the target area to render right away, so that navigation does not stall waiting for offscreen highlighting.
- As a maintainer, I want a reusable background worker framework, so that future deferred tasks can share the same scheduling, cancellation, and completion behavior.
- As a maintainer, I want stale background work to be ignored after edits, so that background jobs cannot overwrite newer editor state.

## Functional Requirements
- [ ] **REQ-001**: The editor shall provide an internal job framework for deferred editor jobs.
- [ ] **REQ-002**: The framework shall accept multiple job kinds so that syntax catch-up and future deferred tasks can share the same scheduling path.
- [ ] **REQ-003**: The framework shall execute jobs on a single serial worker thread.
- [ ] **REQ-004**: The framework shall support job priority tiers so that foreground-adjacent work can run before lower-priority maintenance work.
- [ ] **REQ-005**: The framework shall allow jobs to read editor state from synchronized shared access or equivalent safe state access.
- [ ] **REQ-006**: The framework shall detect stale work using a generation, version, or equivalent cancellation token.
- [ ] **REQ-007**: The framework shall ignore or discard completed results that no longer match the active editor state.
- [ ] **REQ-008**: The framework shall notify the editor loop when job work completes in a way that should trigger a redraw.
- [ ] **REQ-009**: The syntax highlighter shall render the visible viewport immediately without waiting for the rest of the file to be highlighted.
- [ ] **REQ-010**: The syntax highlighter shall continue computing highlight data for offscreen lines after the initial visible render.
- [ ] **REQ-011**: If the user edits text or changes syntax-relevant state, the syntax job shall restart from the earliest affected line or be canceled and resubmitted.
- [ ] **REQ-012**: If a user jumps into a region that has not yet been highlighted by the background worker, the editor shall still render the region immediately using available base styling and fill in syntax styling later.
- [ ] **REQ-013**: Syntax highlighting shall remain disabled when the existing syntax setting is disabled.
- [ ] **REQ-014**: The job framework shall be usable for future deferred tasks without requiring a new scheduling subsystem for each task type.

## Non-Functional Requirements
- **Performance**: Large-file opening and long-distance cursor jumps should avoid blocking on full-file syntax computation.
- **Reliability**: Background jobs must not corrupt editor state, apply stale results, or race with edits.
- **Compatibility**: Existing syntax highlighting behavior for visible text should remain correct once background work catches up.
- **Usability**: Users should observe faster perceived responsiveness without needing new startup flags or manual controls.

## Acceptance Criteria
- [ ] **AC-001**: Opening a large syntax-highlighted file shows the visible viewport quickly even when the file contains many offscreen lines.
- [ ] **AC-002**: Jumping directly to the end of a large file no longer blocks on tokenizing the entire file before the target viewport appears.
- [ ] **AC-003**: Background syntax results eventually fill in offscreen lines after the initial frame without requiring further user input.
- [ ] **AC-004**: An edit that invalidates syntax state causes stale background work to be ignored and fresh work to be scheduled.
- [ ] **AC-005**: Background worker completion triggers a redraw so newly available highlight data becomes visible promptly.
- [ ] **AC-006**: Future background job types can be added without changing the core worker scheduling model.

## Out of Scope
- Tree-sitter or parser-based highlighting.
- User-facing background worker configuration.
- Multiple concurrent worker threads.
- Distributed or cross-process background execution.
- Runtime hot reloading of syntax definitions.

## Assumptions
- A single serial worker thread is sufficient for the first version.
- The syntax highlight pass may be eventually consistent for offscreen content as long as the visible viewport is correct immediately.
- The main editor thread remains responsible for applying accepted results to live state.
- No new user-facing configuration is required for the initial framework.

## Dependencies
- Existing buffer and syntax highlighting code in `src/buffer/syntax.rs`.
- Existing render and event-loop flow in `src/window/view.rs`, `src/window/mod.rs`, and `src/main.rs`.
- Existing global buffer access patterns in `src/globals.rs` and `src/buffer/pool.rs`.
- Existing syntax metadata and registry behavior in `src/syntax/`.
