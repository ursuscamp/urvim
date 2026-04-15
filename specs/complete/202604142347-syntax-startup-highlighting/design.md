# Syntax Startup Highlighting - Technical Design
## Architecture Overview
This change keeps syntax highlighting buffer-owned and policy-driven. The buffer continues to own its text and syntax cache, while startup code decides when to request synchronous or background warmup for each loaded buffer.

The key behavioral split is:
- visible buffers at startup, when their viewport begins at line 0, synchronously warm the first visible lines before the first paint
- visible buffers then receive foreground-priority catch-up so the rest of the current view warms before hidden buffers
- all other loaded buffers queue background syntax catch-up at startup

The implementation should prefer borrowing existing line text or sharing the current `Arc<str>` handles instead of cloning full line contents into new owned strings.
For background catch-up snapshots, the design should keep the persistent line container intact when possible instead of flattening it into a `Vec`, because the job only needs shared line handles.

## Interface Design
The existing buffer syntax methods remain the core API, with two policy-level additions expected in the startup path:

- a startup coordinator that iterates over loaded buffers in the pool
- a visibility-aware warmup decision that selects synchronous or background work

Relevant existing interfaces:

```rust
Buffer::request_syntax_catch_up(buffer_id: BufferId)
Buffer::ensure_syntax_through(line: usize)
Buffer::syntax_spans_for_line(line: usize) -> Option<Vec<SyntaxSpan>>
Buffer::syntax_cache_complete() -> bool
```

Expected behavior for startup coordination:

- if a buffer is visible and begins at line 0, call the synchronous warmup path for the visible range before the initial draw
- after the synchronous warmup completes, queue or promote the visible buffer for foreground catch-up so it stays ahead of hidden buffers
- otherwise request background catch-up

The startup coordinator should not require a new public syntax API unless the current buffer methods prove insufficient.

## Data Models
No new persisted data model is required.

The existing syntax cache continues to store:
- syntax name
- per-line syntax spans
- continuation state needed to tokenize later lines consistently

The buffer line store continues to use `Vector<Arc<str>>`, which is already memory-efficient for shared ownership and supports cheap cloning of line handles for deferred work.
Startup warmup jobs should therefore accept the persistent line container as the snapshot representation and only materialize borrowed line slices at the point where the tokenizer actually requires them.

## Key Components
### Buffer Pool Startup Coordinator
- Enumerates all loaded buffers during startup.
- Determines which loaded buffers are visible at startup.
- Requests synchronous warmup for the visible top-of-file case.
- Requests background catch-up for all other loaded buffers.

### Buffer Syntax Cache
- Preserves the existing cache invalidation and line-by-line tokenization model.
- Supports warming a prefix synchronously without forcing full-file computation on the paint path.
- Accepts background catch-up results only when the generation still matches.

### Window Render Path
- Continues to consume cached spans during rendering.
- May still request catch-up on demand, but startup should already have queued or completed the initial warmup.
- For the top-of-file visible case, should be able to render the first visible lines from the already-warmed cache.

## User Interaction
Users should see faster and more stable first paint when opening urvim. The top of a visible file should appear syntax-highlighted immediately when it begins at line 0, while the rest of the loaded buffers warm up in the background.

There are no new commands or settings for the initial version.

## External Dependencies
No new external dependencies are required. The change uses existing job scheduling, buffer ownership, and syntax tokenization code.

## Error Handling
- If background syntax warmup fails to submit, the editor should continue rendering without crashing and may fall back to on-demand highlighting.
- If a warmup result arrives for an outdated generation, it should be discarded.
- If a buffer changes during startup warmup, the buffer invalidation path should force a fresh generation and ignore stale results.
- If a buffer has no syntax definition, warmup should succeed as a no-op rather than becoming an error path.

## Security
No new security-sensitive behavior is introduced. The feature only rearranges when local syntax classification work occurs.

## Configuration
No new configuration is required for the initial change. The feature should respect the existing syntax-enabled setting and continue to skip work when syntax highlighting is disabled.

## Component Interactions
1. Startup loads buffers into the buffer pool.
2. Startup enumerates loaded buffers and determines which ones are visible.
3. Visible top-of-file buffers warm the first visible region synchronously.
4. All remaining loaded buffers queue background syntax catch-up.
5. The render path reads cached spans for visible lines.
6. Background results update the cache only if the generation still matches.

## Platform Considerations
The design should remain portable across the current terminal-based runtime because it depends only on in-process buffer state and the existing job framework.

The memory-efficiency goal favors retaining `Arc<str>` line sharing instead of duplicating line text into separate owned strings during syntax warmup. If future profiling shows a specific bottleneck, that should be measured separately rather than pre-optimized away.
