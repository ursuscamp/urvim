# Plugin Roadmap Phase 2 Plan

Phase 2 adds core Vimscript-style BearScript APIs for common editor manipulation. These APIs are synchronous, run on the main editor thread, and should remain narrow enough to complete quickly inside normal plugin callbacks.

## Goals

- Add stable namespaced APIs under `urvim.buffers`, `urvim.windows`, `urvim.selection`, `urvim.registers`, `urvim.commands`, `urvim.keymaps`, `urvim.diagnostics`, and `urvim.ui`.
- Prefer plain ids, maps, lists, strings, numbers, booleans, and `null` over live editor handles.
- Keep positions and ranges 0-based for script-facing APIs.
- Reuse existing editor operations where possible instead of duplicating editor behavior inside plugin host functions.
- Keep APIs synchronous and main-thread-only.
- Preserve phase 1 timing instrumentation around callbacks that call these APIs.

## Non-Goals

- Do not add broad worker-thread execution.
- Do not add jobs, timers, providers, syntax plugins, formatters, completion providers, or async filesystem APIs.
- Do not make LSP requests synchronous.
- Do not add a permissions model.
- Do not add compatibility wrappers for the old process/RPC plugin protocol.
- Do not expose long-lived mutable buffer, window, or selection handles to BearScript.

## Design Rules

- Host functions should borrow editor state only for the duration of the native function call.
- Mutating APIs should return clear errors when ids are missing, rows are out of bounds, or arguments have invalid shapes.
- Read APIs should return `null` only when absence is expected, such as no active buffer or no path.
- Invalid ids and invalid argument shapes should produce errors, not silent no-ops.
- API names should stay namespaced unless a top-level alias is clearly worth the extra surface area.
- Start with command-string APIs before callback-backed APIs where both are possible.
- Keep every namespace independently testable.

## Implementation Order

### 1. Host API Structure

Create a small internal structure for building the `urvim` BearScript module so phase 2 does not keep growing one mixed-responsibility function.

Suggested shape:

- Keep `crates/urvim/src/plugin.rs` as the runtime entry point.
- Add focused helpers for namespace construction, such as `buffers_module()`, `windows_module()`, and `diagnostics_module()`.
- Keep value conversion helpers close to plugin host code until there is a second consumer.
- Add shared helpers for parsing ids, positions, ranges, optional strings, and maps from BearScript `Value`.

Acceptance criteria:

- Existing phase 1 APIs still exist.
- `urvim.events` remains unchanged.
- Module construction is split enough that adding a namespace does not make `urvim_module()` harder to read.

### 2. Buffers API

Implement buffers first because most other namespaces depend on buffer ids and buffer text operations.

Initial API:

- `urvim.buffers.active() -> buffer_id | null`
- `urvim.buffers.list() -> [buffer_id]`
- `urvim.buffers.exists(buffer_id) -> bool`
- `urvim.buffers.name(buffer_id) -> string`
- `urvim.buffers.path(buffer_id) -> string | null`
- `urvim.buffers.filetype(buffer_id) -> string`
- `urvim.buffers.set_filetype(buffer_id, filetype)`
- `urvim.buffers.is_modified(buffer_id) -> bool`
- `urvim.buffers.line_count(buffer_id) -> number`
- `urvim.buffers.line(buffer_id, row) -> string`
- `urvim.buffers.lines(buffer_id, start_row, end_row) -> [string]`
- `urvim.buffers.text(buffer_id) -> string`
- `urvim.buffers.set_line(buffer_id, row, text)`
- `urvim.buffers.insert_line(buffer_id, row, text)`
- `urvim.buffers.delete_line(buffer_id, row)`
- `urvim.buffers.replace_range(buffer_id, range, text)`
- `urvim.buffers.save(buffer_id)`

Defer:

- `urvim.buffers.reload(buffer_id)` unless there is already a safe reload path.

Key decisions:

- `active()` should return `null` when no active buffer exists, replacing the older flat `urvim.active_buffer()` behavior if needed.
- `lines(start_row, end_row)` should use an exclusive `end_row`.
- `replace_range` should use `{ "start": { "row", "col" }, "end": { "row", "col" } }`.
- Save should enqueue the same editor events and LSP save notification behavior as normal saves.

Tests:

- Read active/list/existence APIs.
- Read line, lines, text, path, name, filetype, modified state.
- Mutate with set, insert, delete, replace range.
- Error on missing buffer id and out-of-range row.
- Confirm save uses the same save path as editor commands where practical.

### 3. Windows API

Add the visible editor view API after buffers. Use the user-facing term `windows` even if internals use panes, views, or tabs.

Initial API:

- `urvim.windows.active() -> window_id | null`
- `urvim.windows.list() -> [window_id]`
- `urvim.windows.buffer(window_id) -> buffer_id`
- `urvim.windows.cursor(window_id) -> { row, col }`
- `urvim.windows.set_cursor(window_id, row, col)`
- `urvim.windows.visible_range(window_id) -> { start_row, end_row }`
- `urvim.windows.open_buffer(buffer_id)`

Defer:

- `urvim.windows.close(window_id)` until close semantics for the active layout are explicit.

Key decisions:

- If the editor does not have stable window ids yet, add an internal stable id layer before exposing this API.
- Cursor rows and columns are 0-based.
- `open_buffer` should mirror normal editor behavior for showing an existing buffer.

Tests:

- Active/list return stable ids.
- Buffer lookup returns the visible buffer id.
- Cursor read/write clamps or errors consistently with existing editor behavior.
- Visible range matches current viewport state.

### 4. Selection API

Implement selection APIs against the active window first. Do not add per-window selection arguments until there is a concrete need.

Initial API:

- `urvim.selection.get() -> range | null`
- `urvim.selection.text() -> string | null`
- `urvim.selection.set(range)`
- `urvim.selection.clear()`
- `urvim.selection.replace(text)`

Key decisions:

- Range shape should match buffer range shape.
- `text()` returns `null` when there is no active selection.
- `replace()` should no-op or error when there is no active selection; choose one and document it before implementation. Prefer an error for script mistakes.

Tests:

- No-selection behavior.
- Set and clear selection.
- Extract selected text.
- Replace selected text and verify buffer content/cursor state.

### 5. Registers API

Add register access once buffer editing exists, so plugins can integrate with normal editing behavior.

Initial API:

- `urvim.registers.get(name) -> string`
- `urvim.registers.set(name, value)`
- `urvim.registers.append(name, value)`
- `urvim.registers.names() -> [string]`

Key decisions:

- Validate register names using the same rules as editor commands.
- Missing registers should return an empty string if that matches editor behavior; otherwise return `null`. Document the chosen behavior.
- Clipboard-backed registers should use existing register plumbing, not direct OS clipboard calls from plugin code.

Tests:

- Set/get/append named registers.
- List register names.
- Invalid register names error.

### 6. Commands API

Add namespaced command helpers after buffer/window basics. Keep generic command execution carefully bounded because it can re-enter command handling.

Initial API:

- `urvim.commands.register(name, callback, description?)`
- `urvim.commands.unregister(name)`
- `urvim.commands.list() -> [command]`

Optional in phase 2 if reentrancy is resolved:

- `urvim.command(command_line)`
- `urvim.commands.execute(command_line)`

Key decisions:

- `urvim.commands.register` should be an alias for existing `urvim.register_command` semantics, not a second registry.
- `list()` should include plugin-local dynamic commands and enough metadata for discoverability.
- Generic command execution must avoid nested mutable borrows and should use the same parser/executor path as user-entered commands.

Tests:

- Register/unregister through the namespaced API.
- List registered commands.
- If execution is included, verify command parser errors and normal command effects.

### 7. Keymaps API

Start with command-string mappings only. Callback-backed keymaps can wait until callback dispatch semantics are explicit.

Initial API:

- `urvim.keymaps.set(mode, lhs, rhs, opts?)`
- `urvim.keymaps.delete(mode, lhs)`
- `urvim.keymaps.list(mode?) -> [keymap]`

Defer:

- Function callback keymaps.

Key decisions:

- `rhs` is a command string in phase 2.
- Validate mode and lhs using existing keymap config behavior.
- `opts` can start minimal. Only add fields already supported by the editor.

Docs:

- Update `docs/config.md` if plugin-set keymaps interact with user config semantics.

Tests:

- Set/delete/list for normal mode.
- Invalid mode or invalid lhs errors.
- Plugin keymap invokes the configured command string.

### 8. Diagnostics API

Add diagnostics once range conversion helpers are in place.

Initial API:

- `urvim.diagnostics.set(namespace, buffer_id, diagnostics)`
- `urvim.diagnostics.clear(namespace, buffer_id)`
- `urvim.diagnostics.get(buffer_id, namespace?) -> [diagnostic]`
- `urvim.diagnostics.counts(buffer_id) -> map`

Diagnostic shape:

```bear
{
    "range": {
        "start": { "row": 0, "col": 0 },
        "end": { "row": 0, "col": 5 },
    },
    "severity": "error",
    "message": "expected ;",
    "source": "my-linter",
}
```

Key decisions:

- Namespace should be plugin-owned. Consider automatically prefixing or validating against the calling plugin id.
- Severity should accept only known values.
- Setting diagnostics should enqueue or trigger existing diagnostics-changed behavior.

Tests:

- Set/get/clear diagnostics.
- Filter by namespace.
- Counts by severity.
- Invalid ranges and severities error.

### 9. UI API

Keep UI APIs minimal and synchronous. Add only APIs that can be implemented without new async scheduling.

Initial API:

- `urvim.ui.show_message(message, opts?)`

Defer:

- `urvim.ui.input(opts)` unless the editor already has synchronous input prompting that can safely run during plugin callbacks.
- `urvim.ui.select(items, opts)` unless there is an existing synchronous picker/select primitive.
- `urvim.ui.open_picker(opts)` until provider/picker lifecycle is defined.

Key decisions:

- `show_message` should reuse notification plumbing.
- Options can start with `level` only.

Tests:

- Show info/warn/error messages.
- Invalid levels error or fall back consistently with `urvim.notify`.

## Documentation Work

- Update `docs/plugins.md` as each namespace becomes available.
- Add a quick reference table once at least buffers, windows, registers, and diagnostics are implemented.
- If the API surface becomes too large for `docs/plugins.md`, create `docs/plugin-api.md` and link to it.
- Keep examples short and avoid implying deferred APIs are already implemented.

## Test Strategy

- Add BearScript integration tests that call the public API from a plugin.
- Add Rust unit tests for conversion helpers and host operations.
- Prefer small plugin fixtures created in temp directories for runtime tests.
- Test both success and error paths for each namespace.
- Run `cargo fmt`, relevant plugin tests, and `cargo check` after each namespace slice.

## Suggested Milestones

1. Namespace scaffolding and conversion helpers.
2. Buffers read APIs.
3. Buffers mutation APIs.
4. Windows active/cursor APIs.
5. Selection and registers APIs.
6. Commands and keymaps APIs.
7. Diagnostics APIs.
8. Minimal UI APIs.
9. Docs, examples, and cleanup.

## Phase 2 Completion Checklist

- All implemented APIs are reachable under namespaced `urvim.*` modules.
- Top-level aliases are limited to existing phase 1 APIs and any explicitly approved additions.
- Buffer/window/selection positions are documented as 0-based.
- Plugin callback timing still records callbacks that invoke phase 2 APIs.
- `plugin status` still works after API expansion.
- `docs/plugins.md` matches the implemented API surface.
- `cargo check` passes.
- Relevant plugin API tests pass.
