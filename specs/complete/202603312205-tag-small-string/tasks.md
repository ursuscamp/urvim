# Tag Small String - Implementation Tasks

## Overview

Replace `Tag`'s internal `String` storage with a small-string type that supports cheap cloning, then verify that the public API and validation behavior remain unchanged.

## Backend

- [x] **1.** Evaluate and add the chosen small-string crate to `Cargo.toml` and lockfile usage.
  - [x] **1.1** Prefer `smol_str` because it provides `O(1)` cloning and keeps the `Tag` API string-like.
  - [x] **1.2** Avoid candidates that still make clone cost scale with string length.
- [x] **2.** Update `src/theme/tag.rs` to store the validated tag text in the new small-string type.
  - [x] **2.1** Keep `Tag::parse`, `Tag::as_str`, `Tag::parent_chain`, and `TagError` behavior unchanged.
  - [x] **2.2** Preserve ordering, hashing, equality, and display behavior for existing call sites.

## Testing

- [x] **3.** Extend or adjust `src/theme/tag.rs` tests to confirm valid parsing, invalid parsing, and parent-chain behavior still work.
  - [x] **3.1** Add a regression check that cloned `Tag` values remain usable across the existing syntax/theme call patterns.
- [x] **4.** Run `cargo check` and the relevant test subset to verify the refactor builds cleanly and does not change observable behavior.

## Completion Summary

| Item | Status | Notes |
| --- | --- | --- |
| Dependency choice | Complete | `smol_str` added |
| Tag storage update | Complete | Internal field now uses `SmolStr` |
| Test coverage | Complete | Added clone regression test |
| Build verification | Complete | `cargo check` and targeted `cargo test` passed |
