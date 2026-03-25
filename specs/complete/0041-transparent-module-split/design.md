# Transparent Module Split - Technical Design

## 1. Architecture Overview

This refactor keeps urvim's external module surface stable while moving large, mixed-responsibility files into directory-backed modules with focused internal files. The top-level modules continue to expose the same primary types and functions, while internal behavior is reorganized by responsibility.

### Current State
- `src/buffer.rs` mixes buffer state, editing operations, cursor traversal, word motions, text objects, undo history, Unicode helpers, file I/O, and tests.
- `src/window.rs` mixes geometry, viewport state, rendering preparation, gutter logic, action dispatch, count handling, and tests.
- `src/editor.rs` mixes action definitions, count parsing, keymap structures, mode behavior, normal-mode bindings, insert-mode bindings, and tests.
- `src/terminal/mod.rs` mixes terminal lifecycle, output helpers, cursor control, input parsing, bracketed paste handling, test backend code, and tests.

### Target State
- Replace each large file with a directory-backed module.
- Keep public entry points concentrated in the top-level `mod.rs`.
- Split internal responsibilities into focused sibling files.
- Preserve current call sites through re-exports and internal visibility.
- Move large test blocks into module-specific test files where useful.

### Key Architectural Decision

Use transparent internal module extraction instead of interface redesign. This keeps behavior stable and avoids coupling a readability refactor with broader architectural change.

## 2. Interface Design

The external interface remains intentionally shallow and stable.

| Interface | Input | Output | Description |
|-----------|-------|--------|-------------|
| `buffer` module | existing buffer editing and navigation calls | unchanged | Keeps `Buffer`, `Cursor`, `Boundary`, and text-object related items available from `buffer` |
| `window` module | existing window rendering and action processing calls | unchanged | Keeps `Window`, `BufferView`, and related public types available from `window` |
| `editor` module | existing action, mode, and keymap calls | unchanged | Keeps `Action` and mode/keymap-facing APIs available from `editor` |
| `terminal` module | existing terminal lifecycle and event calls | unchanged | Keeps terminal-facing public API available from `terminal` |

Planned top-level layout:

```rust
// Representative structure only
src/
  buffer/
    mod.rs
    edit.rs
    cursor.rs
    boundary.rs
    text_object.rs
    undo.rs
    io.rs
    unicode.rs
    tests.rs
```

## 3. Data Models

This refactor does not intentionally change the meaning of existing data models. It reorganizes where their logic lives.

| Model | Current Location | Planned Top-Level Owner | Notes |
|------|------------------|-------------------------|-------|
| `Buffer` | `src/buffer.rs` | `src/buffer/mod.rs` | Core buffer state remains top-level; behavior moves into focused impl files |
| `Cursor` | `src/buffer.rs` | `src/buffer/mod.rs` | Remains part of buffer-facing API |
| `Boundary` | `src/buffer.rs` | `src/buffer/mod.rs` | Boundary motion API remains stable |
| `TextObjectRange` and related text-object types | `src/buffer.rs` | `src/buffer/mod.rs` | Public items stay re-exported if currently visible |
| `Window` / `BufferView` | `src/window.rs` | `src/window/mod.rs` | State stays centralized; impl blocks split by responsibility |
| `Action` | `src/editor.rs` | `src/editor/mod.rs` | Public action surface remains stable |
| `Terminal` and terminal event types | `src/terminal/mod.rs` | `src/terminal/mod.rs` | Public terminal API remains stable |

### Schema Changes

No persisted schema or file format changes are planned.

## 4. Key Components

### Buffer Module Split

**Responsibilities:**
- Keep buffer-facing public API stable.
- Separate edits, navigation, boundaries, text objects, undo, and I/O into focused files.

**Public API:**
- `pub struct Buffer`
- `pub struct Cursor`
- `pub enum Boundary`
- Existing public text-object items

**Planned Internal Files:**
- `buffer/edit.rs` - insertion, deletion, join, line mutation
- `buffer/cursor.rs` - cursor movement and positioning helpers
- `buffer/boundary.rs` - word and big-word traversal
- `buffer/text_object.rs` - text-object selection logic
- `buffer/search.rs` - character search and repeat helpers if already isolated
- `buffer/undo.rs` - undo/redo state transitions
- `buffer/io.rs` - load/save operations
- `buffer/unicode.rs` - grapheme and visual-column helpers

**Dependencies:**
- `imbl`, Unicode helpers, existing internal line storage

### Window Module Split

**Responsibilities:**
- Keep `Window` ownership centralized while moving concern-specific impl blocks out of one file.

**Public API:**
- `pub struct Window`
- `pub struct BufferView`
- Existing public window-facing methods

**Planned Internal Files:**
- `window/geometry.rs` - positions, sizes, and layout helpers
- `window/view.rs` - viewport and scroll behavior
- `window/render.rs` - render data assembly and screen projection
- `window/gutter.rs` - gutter width and gutter cell rendering
- `window/motions.rs` - movement-oriented action helpers
- `window/commands.rs` - editing and count-processing helpers
- `window/widget_impl.rs` - `Widget` integration if that boundary is currently mixed in

**Dependencies:**
- `buffer`, `screen`, `widget`, and `editor::Action`

### Editor Module Split

**Responsibilities:**
- Separate action definitions, keymap structures, count parsing, and mode-specific bindings.

**Public API:**
- `pub enum Action`
- Existing mode traits/types
- Existing keymap-facing APIs

**Planned Internal Files:**
- `editor/action.rs` - `Action` and action metadata
- `editor/keymap.rs` - trie and shared keymap machinery
- `editor/count.rs` - leading-count and sub-count parsing logic
- `editor/mode.rs` - mode trait and shared mode behavior
- `editor/normal.rs` - normal-mode bindings
- `editor/insert.rs` - insert-mode bindings
- `editor/tests.rs` - editor-focused tests if split from inline blocks

**Dependencies:**
- `motion` helpers, `terminal::keys`, and `window`

### Terminal Module Split

**Responsibilities:**
- Preserve the current terminal API while extracting lifecycle, input, output, and test-specific code.

**Public API:**
- Existing public `Terminal` methods
- Existing terminal event and helper types already exposed from `terminal`

**Planned Internal Files:**
- `terminal/lifecycle.rs` - setup, teardown, raw-mode transitions
- `terminal/output.rs` - write helpers, cursor visibility, screen write plumbing
- `terminal/input.rs` - event polling and bracketed-paste state machine
- `terminal/cursor.rs` - cursor movement helpers if useful to isolate
- `terminal/test_backend.rs` - fake terminal/test harness support
- `terminal/tests.rs` - extracted test blocks

**Dependencies:**
- `rustix`, escape parsing, key parsing, terminal sizing helpers

## 5. User Interaction

This is an internal refactor. User interaction should not change.

### Invocation Patterns
- Normal editing commands keep the same behavior.
- Terminal startup, shutdown, and input parsing keep the same behavior.
- Existing tests and manual editor workflows remain the primary verification path.

### Flows
1. A caller continues importing from the same top-level module path.
2. The top-level `mod.rs` re-exports or owns the same public items.
3. Internal `impl` blocks dispatch into extracted files.
4. Runtime behavior remains unchanged.

## 6. External Dependencies

| Dependency | Purpose | Version/Notes |
|------------|---------|---------------|
| `imbl` | Buffer line storage | Existing dependency |
| `unicode-segmentation` | Grapheme-aware editing behavior | Existing dependency |
| `unicode-width` | Visual width calculations | Existing dependency |
| `rustix` | Terminal I/O and termios handling | Existing dependency |
| existing test harnesses | Regression detection | No new dependency planned |

No new third-party crates are required for this refactor.

## 7. Error Handling

| Error Code | Condition | Error Data | Recovery |
|------------|-----------|------------|----------|
| `BEHAVIOR_REGRESSION` | Refactor changes externally visible behavior | affected command/test | Revert stage or adjust extraction boundary |
| `VISIBILITY_MISMATCH` | Extracted code cannot access required internals | item/module name | Introduce `pub(super)` helper or move seam |
| `CIRCULAR_DEPENDENCY` | New sub-modules depend on each other incorrectly | module pair | Re-center shared helpers in `mod.rs` or dedicated utility module |
| `TEST_DRIFT` | Tests no longer match preserved behavior expectations | test name | Update structure, not semantics; add focused regression tests |

Implementation should prefer moving code without semantic edits. If a seam is not clean, shared private helpers should be extracted before moving larger blocks.

## 8. Security

| Concern | Approach |
|---------|----------|
| Behavior preservation | Avoid combining module splits with logic changes |
| File I/O safety | Keep existing buffer and terminal file handling semantics unchanged |
| Logging | Preserve current logging behavior and avoid introducing extra sensitive output |

This refactor has minimal direct security impact because it is internal-only.

## 9. Configuration

No new configuration is required.

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| None | - | - | - | No config changes planned |

## 10. Component Interactions

```text
main
  -> editor (actions, modes, keymaps)
  -> window (action execution, viewport, rendering)
  -> buffer (text state, cursor, edits, motions)
  -> terminal (input/output, resize, paste)

After refactor:
main
  -> same top-level modules
top-level modules
  -> internal sub-modules by responsibility
```

Important interaction rules:
- `window` continues to depend on `buffer` semantics and `editor::Action`.
- `editor` continues to define action-level intent rather than window execution details.
- `terminal` continues to isolate terminal protocol details from editing logic.
- `main` remains unchanged except for import adjustments if needed.

## 11. Platform Considerations

| Platform | Consideration | Approach |
|----------|---------------|----------|
| macOS | Terminal behavior must remain stable | Preserve current terminal code paths during extraction |
| Linux | Termios and escape handling must remain stable | Keep parsing logic grouped and covered by tests |

No platform-specific behavior changes are intended.

## 12. Trade-offs

**Decision**: Prefer directory-backed modules over introducing new abstraction layers.

**Reasoning:**
- Improves discoverability quickly.
- Minimizes behavior risk.
- Keeps existing ownership and call patterns mostly intact.

**Impact:**
- Some top-level `mod.rs` files remain moderately sized because they own public exports.
- Internal visibility management becomes more explicit.

**Decision**: Stage module splits one target at a time.

**Reasoning:**
- Reduces regression surface.
- Keeps reviews smaller.
- Makes it easier to bisect problems.

**Impact:**
- The full cleanup takes multiple implementation steps.
- Temporary mixed layouts may exist during rollout.

## 13. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `buffer` extraction introduces subtle cursor or text-object regressions | Medium | High | Split helper groups first, preserve tests, add targeted regression tests before and after moves |
| `window` split obscures shared mutable state | Medium | High | Keep state types in `window/mod.rs`; move only cohesive `impl` blocks initially |
| `editor` split breaks action imports or mode construction | Low | Medium | Re-export public action and mode items from `editor/mod.rs` |
| `terminal` split breaks input state machine behavior | Medium | High | Keep input parsing and paste handling together in one extracted file and verify with existing tests |
| Large inline test moves create noisy diffs | Medium | Medium | Move tests in separate commits or stages after behavior-preserving code moves |

## 14. Implementation Order

1. Convert each target file to a directory-backed module while preserving the top-level public API.
2. Split `editor` first, because its responsibilities are conceptually separable and its public surface is clear.
3. Split `window` next by moving cohesive `impl` blocks into dedicated files.
4. Split `terminal/mod.rs` after that, keeping the input state machine together.
5. Split `buffer` last, because it has the most internal coupling and highest regression risk.
6. Move large inline tests into sibling test modules where it reduces file size without weakening coverage.

## 15. Verification Strategy

- Run `cargo check` after each stage.
- Run targeted tests for the module being split before moving to the next stage.
- Run full `cargo test` after completing each target module split or at major checkpoints.
- Manually verify representative editing flows when buffer or window boundaries move.
- Confirm imports and public item paths remain stable for downstream modules.
