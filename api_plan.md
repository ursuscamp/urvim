# Plugin API Future Work Plan

## Current assessment

The plugin API is already capable for synchronous editor extensions:

- Buffer reads/mutations, selections, registers, diagnostics
- Pane and plugin-pane management
- Virtual text and edit-tracking highlights
- Commands, keymaps, themes, syntax providers, and filetypes
- UI overlays, input, confirmation, pickers, and formatting
- Detailed lifecycle events with source payloads
- Async filesystem, jobs, and timers
- Cross-plugin RPC and events

The API is substantially beyond the original `docs/phase2.md` plan. The main risk is now consistency and lifecycle semantics rather than raw surface area.

## Highest-value missing APIs

### 1. Undo/redo and edit transactions

Plugins can mutate buffers, but cannot control how edits appear in undo history.

Suggested APIs:

```text
urvim.buffers.undo(buffer_id)
urvim.buffers.redo(buffer_id)
urvim.buffers.can_undo(buffer_id)
urvim.buffers.can_redo(buffer_id)
urvim.buffers.begin_transaction(buffer_id)
urvim.buffers.end_transaction(buffer_id)
```

A transaction/grouping API is important for formatters, refactoring tools, and multi-edit plugins. Existing undo internals are in `crates/urvim_core/src/buffer/undo.rs`.

### 2. Buffer lifecycle and file operations

Current APIs inspect and save existing buffers, but cannot directly open paths, create scratch buffers, save-as, reload, close buffers/tabs, or query buffer generation and disk state.

Suggested APIs:

```text
urvim.buffers.open(path) -> buffer_id
urvim.buffers.create(opts?) -> buffer_id
urvim.buffers.save_as(buffer_id, path)
urvim.buffers.reload(buffer_id)
urvim.buffers.close(buffer_id)
urvim.buffers.generation(buffer_id) -> number
```

The underlying buffer pool already exposes save-to-path and reload functionality.

### 3. Tab APIs

Events expose tab IDs, but plugins cannot inspect or manipulate tabs directly.

Suggested namespace:

```text
urvim.tabs.list(pane_id?) -> [tab]
urvim.tabs.active(pane_id?) -> tab_id | null
urvim.tabs.buffer(tab_id) -> buffer_id
urvim.tabs.activate(tab_id)
urvim.tabs.close(tab_id)
urvim.tabs.move(tab_id, target_pane_id?)
```

This would support navigation, tabline, session, and workspace plugins.

### 4. Better editor state inspection

Plugins can query pane cursor and visible range, but not all commonly needed state.

Suggested APIs:

```text
urvim.editor.mode()
urvim.editor.cwd()
urvim.editor.active_pane()
urvim.editor.active_tab()
urvim.editor.is_recording()
urvim.editor.registered_options()
```

At minimum, expose current mode, working directory, pane geometry, and tab metadata.

### 5. Search APIs

Plugins currently need to implement searching over `buffers.text()` themselves.

Suggested APIs:

```text
urvim.search.find(buffer_id, pattern, opts?) -> [match]
urvim.search.find_in_range(buffer_id, range, pattern, opts?) -> [match]
urvim.search.replace(buffer_id, pattern, replacement, opts?) -> count
```

This would benefit linters, symbol navigation, match highlighting, project tools, and structural editing.

### 6. LSP request APIs

The event API exposes LSP lifecycle, but there is no plugin-facing way to request LSP functionality.

Likely future namespace:

```text
urvim.lsp.hover(opts, callback)
urvim.lsp.definition(opts, callback)
urvim.lsp.references(opts, callback)
urvim.lsp.code_actions(opts, callback)
urvim.lsp.format(opts, callback)
urvim.lsp.completion(opts, callback)
```

These should remain asynchronous and return request IDs, with a clearly defined cancellation and error model.

### 7. Plugin persistence and configuration

Filesystem access exists, but there is no supported plugin-local state/configuration API. Plugins will otherwise invent incompatible storage conventions.

Suggested APIs:

```text
urvim.config.get(path?) -> value
urvim.state.get(key, default?) -> value
urvim.state.set(key, value)
urvim.state.delete(key)
```

State should be serialized, plugin-owned, and restricted to portable values.

### 8. Structured text-edit helpers

`replace_range` is useful but low-level. Common operations would benefit from:

```text
urvim.buffers.insert(buffer_id, position, text)
urvim.buffers.delete(buffer_id, range)
urvim.buffers.apply_edits(buffer_id, edits)
urvim.buffers.text_in_range(buffer_id, range)
```

`apply_edits` should validate and apply edits atomically, ideally as one undo transaction.

## API correctness issues to address first

These are more urgent than adding new namespaces:

1. **Keymap ownership is missing.** `crates/urvim/src/plugin/host/keymaps.rs` stores plugin keymaps in shared global tables. `delete` and `list` are not visibly scoped to the calling plugin, unlike overlays and markers. One plugin may be able to overwrite or delete another plugin's mapping.

2. **Diagnostics namespaces are not visibly plugin-owned.** `diagnostics.set(namespace, ...)` accepts an arbitrary namespace. Documentation says namespaces should be plugin-owned, but the host implementation does not appear to enforce this.

3. **`filetypes.register` and `syntax.register` accept `opts` but ignore them.** Either implement documented options or remove the parameters until semantics exist.

4. **API naming has drifted.** The roadmap refers to `windows`, while implementation and documentation use `panes`. This is reasonable internally, but public terminology should be finalized before more APIs are added.

5. **Mutation consistency needs review.** Buffer mutations should consistently produce source-aware events, update undo state, refresh syntax/LSP state, and preserve cursor/selection behavior.

## Recommended order

1. Fix ownership and lifecycle semantics for keymaps, diagnostics, markers, panes, timers, and jobs.
2. Add edit transactions plus undo/redo.
3. Add `apply_edits`, `insert`, `delete`, and `text_in_range`.
4. Add buffer open/create/save-as/reload APIs.
5. Add tab and richer editor-state APIs.
6. Add search.
7. Design asynchronous LSP requests and cancellation.
8. Add plugin state/configuration.

The API is already broad enough for useful plugins. The next phase should prioritize safe ownership, consistent mutation semantics, and transactional editing rather than adding more UI primitives.
