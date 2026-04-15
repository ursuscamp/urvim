# Syntax Startup Highlighting
## Summary
urvim should precompute syntax highlighting for every buffer already loaded into the buffer pool at startup, while keeping the first visible paint as fast as possible. Buffers that are not currently visible should be highlighted in the background. Buffers that are visible and start at line 0 should synchronously highlight the first visible portion before the initial render completes. The existing `Arc<str>` line storage should remain the default buffer representation unless a concrete memory or performance issue is proven.

## Problem Statement
Today syntax highlighting is primarily demand-driven. The first visible render can show incomplete or missing syntax until the cache warms, and hidden buffers may remain completely unhighlighted until they are eventually drawn. This creates inconsistent startup behavior and shifts work into the critical paint path.

The buffer text model also uses `Arc<str>` per line. The desired behavior should improve startup latency and memory efficiency without introducing unnecessary line duplication or a second ownership model unless it is clearly justified.

## User Stories
- As a user opening urvim, I want my loaded buffers to already be syntax-warmed so that the editor feels ready immediately.
- As a user looking at the first screenful of a file, I want the first visible lines to be highlighted synchronously when the file starts at line 0 so that the initial paint is complete.
- As a user with many loaded but hidden buffers, I want those buffers highlighted in the background so that switching to them later is smooth.
- As a maintainer, I want the text storage strategy to remain memory efficient and simple so that the editor does not accumulate avoidable copies.

## Functional Requirements
- [ ] **REQ-001**: At startup, urvim shall schedule syntax highlighting work for every buffer already loaded into the buffer pool.
- [ ] **REQ-002**: Buffers that are not currently visible at startup shall be highlighted using background-priority work.
- [ ] **REQ-003**: For a buffer that is visible at startup and whose viewport begins at line 0, urvim shall synchronously highlight the first visible portion before the initial render completes.
- [ ] **REQ-004**: Synchronous startup highlighting shall be limited to the visible top-of-file region and shall not eagerly compute the entire file on the render path.
- [ ] **REQ-005**: After the synchronous first-paint warmup, a visible buffer shall receive foreground-priority syntax catch-up so the remainder of its visible syntax can warm ahead of hidden buffers.
- [ ] **REQ-006**: Hidden buffers shall not block the initial visible paint while waiting for syntax completion.
- [ ] **REQ-007**: The syntax cache shall remain invalidation-aware so edits, undo, redo, and filetype changes still force recomputation from the correct line.
- [ ] **REQ-008**: The implementation shall preserve the existing `Arc<str>` per-line buffer representation unless a measurable reason to change it is identified.
- [ ] **REQ-009**: The implementation shall avoid introducing additional full-line string copies for syntax warmup when borrowed line text or shared line handles are sufficient.
- [ ] **REQ-010**: Background syntax warmup shall snapshot buffer lines using the existing persistent line container where practical instead of converting the snapshot into a `Vec` solely for job submission.

## Non-Functional Requirements
- **Performance**: Initial paint should prioritize visible content readiness over complete syntax coverage.
- **Performance**: Visible buffers should continue warming ahead of hidden buffers after the first paint so the current view becomes complete sooner.
- **Memory efficiency**: Startup highlighting should reuse existing line storage and avoid duplicating file contents unnecessarily.
- **Memory efficiency**: Startup highlighting should prefer the existing persistent line container and shared string handles over redundant container conversions.
- **Compatibility**: Existing buffer editing, undo/redo, and syntax cache invalidation behavior shall continue to work.
- **Reliability**: Background syntax warmup shall ignore stale results when a buffer changes before a job completes.

## Acceptance Criteria
- [ ] **AC-001**: When urvim starts with multiple loaded buffers, each buffer has either begun or completed syntax warmup without requiring the user to open it first.
- [ ] **AC-002**: A visible buffer whose viewport starts at line 0 renders its first visible lines with syntax styling on the first frame.
- [ ] **AC-003**: After the initial synchronous highlight, a visible buffer is queued ahead of hidden buffers for further syntax catch-up work.
- [ ] **AC-004**: A non-visible loaded buffer does not delay the initial visible render while syntax warmup is queued.
- [ ] **AC-005**: Editing a buffer after startup still invalidates syntax correctly and recomputes from the earliest affected line.
- [ ] **AC-006**: The buffer text model continues to store lines as shared string slices rather than eagerly copying them into independent owned strings for syntax warmup.

## Out of Scope
- Changing the syntax grammar language or adding new token categories.
- Reworking the renderer’s chunk model beyond what is needed to consume syntax cache data.
- Introducing user-facing configuration for startup syntax priority.
- Pre-highlighting buffers that are not loaded into the buffer pool yet.

## Assumptions
- The buffer pool already has a reliable way to enumerate loaded buffers at startup.
- The current syntax cache and background job system are suitable for startup warmup with only policy changes.
- `Arc<str>` remains an acceptable storage choice for line text because it already supports sharing across undo snapshots and background jobs.
- “Visible buffer” refers to a buffer currently attached to a rendered window or tab at startup.

## Dependencies
- Existing buffer pool ownership and enumeration support.
- Existing syntax cache and background job infrastructure.
- Existing render path that reads cached syntax spans during paint.
