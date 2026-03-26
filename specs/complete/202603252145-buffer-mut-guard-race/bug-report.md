# BR-202603252145: Detached `BufferMutGuard` can lose concurrent edits

## Summary
`BufferMutGuard` clones a `Buffer` out of the global pool and writes the clone back on `Drop`. That makes the guard a detached snapshot rather than an exclusive mutable borrow of the live buffer. If two threads obtain guards for the same `BufferId`, both can mutate separate clones and the last guard to drop overwrites the other thread's changes.

## Severity: High

## Environment
- Repository: `/Users/ryan/Dev/urvim`
- Relevant code:
  - `/Users/ryan/Dev/urvim/src/buffer/pool.rs`
  - `/Users/ryan/Dev/urvim/src/globals.rs`
  - `/Users/ryan/Dev/urvim/src/window/view.rs`
- Observed against the current global buffer pool implementation on `main`

## Reproduction Steps
1. Create a `BufferPool` and register a buffer under a single `BufferId`.
2. Spawn two threads that each call `BufferPool::guard(id)` or `BufferView::buffer_mut()` for the same buffer.
3. In thread A, mutate the cloned buffer to insert text such as `A`.
4. In thread B, mutate the cloned buffer to insert different text such as `B`.
5. Let the guards drop in opposite orders across repeated runs.
6. Observe that the final buffer contents depend on which guard drops last.

## Expected Behavior
Concurrent access to the same buffer should be serialized so that each mutation sees a consistent live buffer state, or conflicting mutable access should be rejected.

## Actual Behavior
Each guard owns its own cloned `Buffer`, so concurrent edits are applied to isolated snapshots. When the guards drop, each one writes its snapshot back through `replace_buffer`, and the last drop wins. Earlier changes can be silently lost.

## Impact
- Lost edits when two tasks mutate the same buffer concurrently
- Non-deterministic final buffer state
- Potential corruption of user work if the editor ever performs buffer mutations from more than one thread
- The current API shape suggests exclusivity, but it does not actually provide it

## Root Cause
`BufferMutGuard` in `/Users/ryan/Dev/urvim/src/buffer/pool.rs` stores a cloned `Buffer` instead of a live borrow or synchronized handle. `BufferPool::guard` clones the buffer while holding only `&self`, which means multiple guards can be created at once for the same `BufferId`. `Drop` then calls `crate::globals::with_buffer_pool` and unconditionally replaces the stored buffer with the guard's snapshot. That is a last-writer-wins design, not true mutual exclusion.

## Solution Approach
Replace the snapshot-and-replace guard with real synchronized mutable access to the pool entry so only one mutable guard for a given buffer can exist at a time.

Preferred fix:
- Make the guard hold exclusive access to the live buffer entry for the duration of the mutation window.
- Commit through that exclusive handle instead of cloning and later overwriting the pool entry.
- Keep the pool-level API responsible for serialization so concurrent callers cannot create conflicting guards for the same `BufferId`.

Rejected alternatives:
- Keep clone-on-drop and accept last-writer-wins semantics. This preserves the race.
- Try to merge snapshots on drop without an explicit conflict model. That would still be non-deterministic and brittle.

## Code Changes
- `/Users/ryan/Dev/urvim/src/buffer/pool.rs`
  - Redesign `BufferMutGuard` so it cannot outlive exclusive access to the live pool entry.
  - Update `BufferPool::guard` to return a synchronized guard instead of a detached clone.
  - Add tests that demonstrate concurrent guards are serialized or rejected.
- `/Users/ryan/Dev/urvim/src/window/view.rs`
  - Adjust `BufferView::buffer_mut` to use the new guard semantics.
- `/Users/ryan/Dev/urvim/src/globals.rs`
  - Update any helper that exposes mutable buffer access so it routes through the synchronized guard path.

## Edge Cases
- A guard dropped after the buffer has been removed from the pool should not silently resurrect stale state.
- Nested or reentrant mutable access to the same `BufferId` should fail clearly rather than deadlock or overwrite.
- Existing single-threaded editing flows should continue to behave exactly as before.
- Read-only access should remain inexpensive and should not block unrelated buffer reads.
