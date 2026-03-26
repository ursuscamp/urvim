# Buffer Read Lock Refactor - Implementation Tasks

## Overview
Refactor the global buffer access layer so reads use closure-scoped live access instead of cloning `Buffer`, and switch the global pool synchronization primitive from `Mutex` to `RwLock`. Update the window, layout, and rendering call sites to use the new read helper, then verify the refactor with targeted tests and `cargo check`.

## Core Refactor
- [ ] **1.** Replace the global `Mutex<BufferPool>` with `RwLock<BufferPool>` and add a closure-based read helper.
  - [ ] **1.1** Update `globals.rs` to expose the new pool type and add `with_buffer(id, f)` for read-only access.
  - [ ] **1.2** Keep `with_buffer_mut(id, f)` working through the write side of the `RwLock`.
  - [ ] **1.3** Remove or replace the clone-returning `get_buffer` API so callers cannot request a detached `Buffer` snapshot.
  - [ ] **1.4** Update any global helper documentation and comments to describe the new lock-scoped read/write model.

- [ ] **2.** Update `BufferView` and buffer-using call sites to consume live reads instead of owned snapshots.
  - [ ] **2.1** Refactor `BufferView` so it no longer depends on `get_buffer` for ordinary read access.
  - [ ] **2.2** Update rendering, cursor math, and motion helpers to use closure-scoped reads for buffer metadata and line text.
  - [ ] **2.3** Update layout and tab group code paths that inspect buffer state to use the new read helper.
  - [ ] **2.4** Keep all existing mutation call sites on the current closure-based write path.

## Testing
- [ ] **3.** Add and update tests to cover the new read-lock behavior.
  - [ ] **3.1** Add unit tests for the new read helper covering existing buffers, missing buffers, and callback return values.
  - [ ] **3.2** Add a concurrency test showing multiple readers can access the pool concurrently while preserving correctness.
  - [ ] **3.3** Add or update mutation tests to confirm writes still change the live buffer and do not depend on cloning.
  - [ ] **3.4** Update any affected window, layout, or buffer tests that previously asserted against owned snapshots.

- [ ] **4.** Run repository validation and fix fallout.
  - [ ] **4.1** Run `cargo check` and address compile errors or warnings introduced by the API change.
  - [ ] **4.2** Run the relevant targeted test modules for buffer, window, layout, and global state behavior.
  - [ ] **4.3** Fix any clippy or style issues surfaced by the refactor.

## Completion Summary
| Area | Status | Notes |
| --- | --- | --- |
| Core Refactor | Pending | Global pool read/write API and `RwLock` migration |
| Call Site Updates | Pending | Window, layout, motion, and rendering read paths |
| Testing | Pending | Read helper, concurrency, and regression coverage |
| Validation | Pending | `cargo check` and targeted test runs |
