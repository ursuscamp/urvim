# Plugins

urvim plugins are explicitly enabled, local BearScript programs. This document is the supported reference for creating and using them.

A plugin directory contains `urvim-plugin.toml` and the BearScript entry file named by its manifest.

## Loading Model

Plugins are never auto-discovered. A plugin is loaded only when it appears in `config.toml` under `[plugins.<plugin-id>]`.

Default path:

```toml
[plugins.demo]
enabled = true
```

When `path` is omitted, urvim loads the plugin from `$XDG_CONFIG_HOME/urvim/plugins/<plugin-id>`.

Explicit path:

```toml
[plugins.demo]
enabled = true
path = "~/src/urvim/plugins/demo"
```

Rules:

- The config table key is the plugin id.
- `enabled` is optional and defaults to `true`.
- `enabled = false` skips the plugin and does not read the plugin path.
- `path` is optional and may start with `~` or `~/`.
- The manifest file is always `urvim-plugin.toml`.
- The manifest `name` must match the configured plugin id.

## Manifest Schema

Example `urvim-plugin.toml`:

```toml
name = "demo"
version = "0.1.0"
entry = "plugin.bear"
```

Fields:

- `name`: required plugin namespace; must match `[plugins.<plugin-id>]`.
- `version`: required non-empty version string.
- `entry`: required BearScript file path relative to the plugin root.

Unknown manifest fields are rejected.

## BearScript Entry

The entry file is evaluated once at startup. After evaluation, urvim calls `init()`.

Plugins dynamically register commands, event hooks, and other contributions from `init()` through the global `urvim` module.

```text
fn init() {
    urvim.commands.register("hello", hello, "Show a greeting")
    let hook = urvim.register_event_hook(urvim.events.BufferSaved, on_buffer_saved)
}

fn hello(args) {
    urvim.ui.show_message("hello from BearScript", { "level": "info" })
}

fn on_buffer_saved(event) {
    urvim.ui.show_message("buffer saved", { "level": "info" })
}
```

Run commands through the plugin namespace:

```text
plugin demo hello
```

Command arguments are passed to the BearScript function as a list of strings.

## Execution Model

BearScript plugins run in-process on the main editor thread. The current synchronous callbacks are:

- `init()`, called during plugin loading.
- Registered commands, called when `plugin <plugin-id> <command>` is executed.
- Registered event hooks, called when editor events are drained.
- Timer callbacks, called when scheduled timer events are drained.

Synchronous callbacks may call editor APIs directly, but they should stay quick. Avoid blocking I/O, long external commands, and expensive CPU work inside `init()`, commands, event hooks, or timer callbacks. Future plugin phases will add provider APIs for expensive structured work.

urvim records callback timing for plugin health. Callbacks at or above these thresholds are considered slow:

- `16ms`: logged as a slow callback.
- `50ms`: logged and shown as a warning notification.
- `100ms`: logged and shown as a warning notification.

Runtime health tracks loaded or failed state, the last callback/load error, slow callback count, total callback count, and callback timing stats.

## Host API Reference

The global `urvim` module exposes the APIs below. All arguments and return values are BearScript values. APIs raise an error for invalid argument shapes or invalid ids unless noted otherwise.

### Editor and UI

- `urvim.ui.show_message(message, opts)`
- `urvim.ui.confirm(opts) -> confirmation_id`
- `urvim.ui.close_confirmation(confirmation_id)`
- `urvim.ui.input(opts) -> input_id`
- `urvim.ui.close_input(input_id)`
- `urvim.ui.overlays.create(opts) -> overlay_id`
- `urvim.ui.overlays.configure(overlay_id, opts)`
- `urvim.ui.overlays.set_content(overlay_id, content)`
- `urvim.ui.line_format.render(opts) -> content`
- `urvim.ui.overlays.show(overlay_id)`
- `urvim.ui.overlays.hide(overlay_id)`
- `urvim.ui.overlays.focus(overlay_id)`
- `urvim.ui.overlays.blur(overlay_id)`
- `urvim.ui.overlays.close(overlay_id)`
- `urvim.ui.overlays.list() -> [overlay_id]`
- `urvim.ui.overlays.active() -> overlay_id | null`
- `urvim.ui.overlays.set_keymap(overlay_id, lhs, rhs)`
- `urvim.ui.overlays.delete_keymap(overlay_id, lhs)`
- `urvim.ui.overlays.list_keymaps(overlay_id) -> [keymap]`
- `urvim.panes.create(target_pane_id, opts) -> pane_id`
- `urvim.panes.configure(pane_id, opts)`
- `urvim.panes.set_content(pane_id, content)`
- `urvim.panes.focus(pane_id)`
- `urvim.panes.close(pane_id)`
- `urvim.panes.list() -> [{ id, kind }]`
- `urvim.panes.active() -> { id, kind } | null`
- `urvim.panes.set_keymap(pane_id, lhs, rhs)`
- `urvim.panes.delete_keymap(pane_id, lhs)`
- `urvim.panes.list_keymaps(pane_id) -> [keymap]`
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
- `urvim.panes.active() -> { id, kind } | null`
- `urvim.panes.list() -> [{ id, kind }]`
- `urvim.panes.buffer(pane_id) -> buffer_id`
- `urvim.panes.cursor(pane_id) -> { row, col }`
- `urvim.panes.set_cursor(pane_id, row, col)`
- `urvim.panes.visible_range(pane_id) -> { start_row, end_row }`
- `urvim.panes.open_buffer(buffer_id)`

### Commands, Keymaps, and Events

- `urvim.commands.register(name, function, description?)`
- `urvim.commands.unregister(name)`
- `urvim.commands.list() -> [command]`
- `urvim.commands.execute(command_line) -> bool`
- `urvim.command(command_line) -> bool`
- `urvim.keymaps.set(mode, lhs, rhs, opts?)`
- `urvim.keymaps.delete(mode, lhs)`
- `urvim.keymaps.list(mode?) -> [keymap]`
- `urvim.register_event_hook(event, function) -> hook_id`
- `urvim.unregister_event_hook(hook_id)`

Keymap modes are `normal`, `insert`, `visual`, `visual_line` (or `visual-line`), and `resizing` (or `resize`). Keymap right-hand sides are urvim command lines. The optional keymap options map is currently reserved and must be empty.

Buffer commands accept a named `buffer=<buffer-id>` argument, allowing ids returned by the buffer and pane APIs to target non-active buffers. BearScript string interpolation can construct the command line:

```text
let buffer_id = urvim.panes.buffer(pane_id)
urvim.command("buffer write buffer={buffer_id}")
urvim.command("buffer filetype rust buffer={buffer_id}")
```

`urvim.command` and `urvim.commands.execute` return whether the command was handled, not whether the requested operation succeeded. Runtime failures such as an unknown buffer id are reported through editor notifications.

### Registers and Themes

- `urvim.registers.get(name) -> string`
- `urvim.registers.set(name, value)`
- `urvim.registers.append(name, value)`
- `urvim.registers.names() -> [string]`
- `urvim.themes.list() -> [theme]`
- `urvim.themes.set(name)`
- `urvim.themes.register(path) -> name`
- `urvim.themes.create(theme) -> name`
- `urvim.themes.unregister(name)`

Register names are a single lowercase ASCII letter or `"`.

### Files, Paths, Environment, and Data

- `urvim.fs.exists(path) -> bool`
- `urvim.fs.is_file(path) -> bool`
- `urvim.fs.is_dir(path) -> bool`
- `urvim.fs.read_file(path, callback) -> request_id`
- `urvim.fs.write_file(path, text, callback) -> request_id`
- `urvim.fs.read_dir(path, callback) -> request_id`
- `urvim.path.join(parts) -> string`
- `urvim.path.dirname(path) -> string`
- `urvim.path.basename(path) -> string`
- `urvim.path.extension(path) -> string | null`
- `urvim.path.stem(path) -> string`
- `urvim.env.get(name) -> string | null`
- `urvim.project.find_up(marker_or_markers, start?) -> string | null`
- `urvim.project.root(marker_or_markers, start?) -> string | null`
- `urvim.json.parse(text) -> value`
- `urvim.json.stringify(value) -> string`
- `urvim.json.stringify_pretty(value) -> string`
- `urvim.inspect(value) -> string`

`project.find_up` returns the matching marker path; `project.root` returns its parent directory. Both search upward from `start`, or the editor process current directory when omitted.

### Filetypes and Syntax

- `urvim.filetypes.register(name, opts?)`
- `urvim.filetypes.detect_extension(extension, filetype)`
- `urvim.syntax.register(filetype, callback, opts?) -> provider_id`
- `urvim.syntax.unregister(provider_id)`
- `urvim.syntax.refresh(buffer_id?)`
- `urvim.syntax.tags() -> [string]`

### Utilities

- `urvim.lists.push(list, value) -> list`
- `urvim.strings.len(text) -> number`
- `urvim.strings.byte_len(text) -> number`
- `urvim.strings.char_at(text, index) -> string | null`
- `urvim.strings.find(text, needle, start?) -> number`
- `urvim.strings.trim(text) -> string`
- `urvim.strings.trim_start(text) -> string`
- `urvim.strings.trim_end(text) -> string`
- `urvim.strings.starts_with(text, prefix) -> bool`
- `urvim.strings.ends_with(text, suffix) -> bool`
- `urvim.strings.contains(text, needle) -> bool`
- `urvim.strings.split(text, separator) -> [string]`
- `urvim.strings.join(parts, separator) -> string`
- `urvim.strings.replace(text, from, to) -> string`
- `urvim.strings.to_lower(text) -> string`
- `urvim.strings.to_upper(text) -> string`

### Jobs and Timers

- `urvim.jobs.spawn(opts) -> job_id`
- `urvim.jobs.kill(job_id)`
- `urvim.jobs.status(job_id) -> string`
- `urvim.jobs.write_stdin(job_id, text)`
- `urvim.jobs.close_stdin(job_id)`
- `urvim.jobs.list() -> [job]`
- `urvim.timers.defer(callback) -> timer_id`
- `urvim.timers.set_timeout(ms, callback) -> timer_id`
- `urvim.timers.set_interval(ms, callback) -> timer_id`
- `urvim.timers.clear(timer_id)`

Event constants are available under `urvim.events`, for example `urvim.events.BufferSaved`.

Notification levels are `info`, `warn`, `warning`, and `error`.

Buffer rows and columns are 0-based. `urvim.buffers.lines(buffer_id, start_row, end_row)` uses an exclusive `end_row`. Ranges use this shape:

```text
{
    "start": { "row": 0, "col": 0 },
    "end": { "row": 0, "col": 5 }
}
```

Invalid buffer ids, rows, columns, and argument shapes raise errors. `urvim.buffers.save(buffer_id)` saves through the normal buffer save path and emits the same `BufferSaved` or `BufferSaveFailed` editor event.

Pane rows and columns are 0-based. Pane ids are stable for each split-tree leaf and become invalid after close. Editor-only methods reject plugin panes. `urvim.panes.open_buffer(buffer_id)` activates an existing visible editor tab or opens a loaded hidden buffer in the active or last-focused editor pane, including while a plugin pane or overlay has focus. The last editor pane cannot be closed.

### Confirmations

`urvim.ui.confirm(opts)` opens a modal binary confirmation and returns its id:

```text
fn handle_response(response) {
    if response == "delete" {
        urvim.ui.show_message("Deleted")
    }
}

fn handle_cancel() {
    urvim.ui.show_message("Dismissed")
}

let confirmation_id = urvim.ui.confirm({
    "title": "Delete file?",
    "message": "This cannot be undone.",
    "confirm": {
        "label": "Delete",
        "key": "d",
        "value": "delete"
    },
    "reject": {
        "label": "Keep",
        "key": "k",
        "value": "keep"
    },
    "on_response": handle_response,
    "on_cancel": handle_cancel
})
```

`message` and `on_response` are required. `title` defaults to `Confirm`. The
`confirm` and `reject` response maps are optional and default to `Yes`/`y`/`true`
and `No`/`n`/`false`. Each response map can override `label`, its
single-character `key`, and any BearScript `value`. Response keys must be
different. Enter selects the confirm response. Selecting either response calls
`on_response` with its unchanged value.

`on_cancel` is optional and is called after Esc,
`urvim.ui.close_confirmation(confirmation_id)`, or replacement by another
dialog. A response or cancellation callback is delivered at most once.
Confirmation ids are owned by the creating plugin and become invalid when the
dialog closes. See `examples/plugins/confirmation-demo` for a complete example.

### Input Dialogs

`urvim.ui.input(opts)` opens a modal single-line input dialog and returns its id:

```text
fn handle_submit(text) {
    urvim.ui.show_message("Entered: {text}")
}

fn handle_cancel() {
    urvim.ui.show_message("Dismissed")
}

let input_id = urvim.ui.input({
    "title": "Rename",
    "prompt": "Name: ",
    "initial": "old_name",
    "on_submit": handle_submit,
    "on_cancel": handle_cancel
})
```

`on_submit` is required. `title` defaults to `Input`; `prompt` and `initial`
default to empty strings. Enter calls `on_submit` with the exact input text,
including an empty string. Pasted line-break runs are converted to spaces so the
value remains single-line.

`on_cancel` is optional and is called after Esc,
`urvim.ui.close_input(input_id)`, or replacement by another dialog. A submit or
cancellation callback is delivered at most once. Input ids are owned by the
creating plugin and become invalid when the dialog closes. See
`examples/plugins/input-demo` for a complete example.

### Custom Pickers

`urvim.ui.pickers` opens a modal picker backed by items supplied by a plugin:

- `urvim.ui.pickers.open(opts) -> picker_id`
- `urvim.ui.pickers.set_items(picker_id, items)`
- `urvim.ui.pickers.append_items(picker_id, items)`
- `urvim.ui.pickers.close(picker_id)`

```text
fn select_branch(branch) {
    urvim.ui.show_message("Selected {branch}")
}

fn cancel_branch_picker() {
    urvim.ui.show_message("No branch selected")
}

let picker_id = urvim.ui.pickers.open({
    "title": "Branches",
    "items": [],
    "on_select": select_branch,
    "on_cancel": cancel_branch_picker
})

urvim.ui.pickers.append_items(picker_id, [{
    "key": "main",
    "label": "main",
    "detail": "origin/main",
    "value": "main"
}])
```

`title` and `items` are optional. `on_select` is required and receives the
selected item's unchanged `value`. `on_cancel` is optional and is called after
Esc, Ctrl-C, `close`, or replacement by another dialog. A picker callback is
delivered at most once.

Each item requires `label` and `value`. `detail` is optional and is displayed
on the right. Labels and details are both searched. The optional string `key`
preserves the highlighted item across `set_items` calls; keys must be unique in
a picker. Without a key, urvim assigns an identity valid only for that update.
Tab switches between fuzzy and exact matching.

`set_items` replaces the current item set and `append_items` adds to it, so job,
filesystem, and timer callbacks can update an open picker. Picker ids are owned
by the creating plugin and become invalid when the picker closes. Only one
modal picker can be open at a time; opening another dialog cancels the current
custom picker.

See `examples/plugins/emoji-picker` for a complete picker that inserts the
selected value into the active buffer.

### Plugin Overlays

`urvim.ui.overlays` creates transient overlays that are separate from
the split-tree panes in `urvim.panes`. They are retained by urvim and
rendered as UI widgets; plugins do not provide a callback that runs during
painting.

Create an overlay with content dimensions and an anchored placement:

```text
let overlay_id = urvim.ui.overlays.create({
    "placement": {
        "type": "anchored",
        "anchor": "top_right",
        "margins": { "top": 1, "right": 2 }
    },
    "rows": 8,
    "cols": 40,
    "title": "My Plugin",
    "body_style": "ui.window",
    "border_style": "ui.window.lines.border",
    "focused_border_style": "ui.window.lines.resize"
})
```

Supported anchors are `center`, `top_center`, `top_right`, and
`bottom_right`. Anchored placement margins optionally accept `top`, `right`,
`bottom`, and `left` values. An omitted or `null` side is treated as zero.
Margins inset the available editor area before the anchor is resolved, so they
apply consistently to every supported anchor.

For fixed placement, `row` and `col` are zero-based coordinates for the
outer frame's top-left corner, relative to the editor UI area:

```text
let overlay_id = urvim.ui.overlays.create({
    "placement": { "type": "fixed", "row": 3, "col": 10 },
    "rows": 8,
    "cols": 40
})
```

Fixed placement does not accept margins. If a fixed frame extends past the
available area, its origin remains fixed and its rows or columns are clipped
to the remaining space. A frame is omitted if fewer than 3 rows or columns
remain. `rows` and `cols` describe the content area; the border is added
outside those dimensions. Content is clipped to the available area and is not
wrapped.

When configuring an existing overlay, supplying `placement` replaces the
complete placement. Omitted anchored margins default to zero; omit
`placement` to leave the current placement unchanged:

```text
urvim.ui.overlays.configure(overlay_id, {
    "placement": {
        "type": "anchored",
        "anchor": "top_right",
        "margins": { "top": 1, "right": 2 }
    }
})
```

Content is a list of lines, where each line is a list of styled segments:

```text
urvim.ui.overlays.set_content(overlay_id, [
    [
        { "text": "hello ", "style": "ui.window" },
        { "text": "world", "style": "syntax.keyword" }
    ],
    [{ "text": "plain text" }]
])
```

Segment styles are named theme tags. The style is optional and defaults to the
overlay body style. Segment text must not contain newlines.

Overlays are created visible but unfocused. An unfocused overlay frame and title
use `border_style`, which defaults to `ui.window.lines.border`; a focused overlay
uses `focused_border_style`, which defaults to `ui.window.lines.resize`. A
focused overlay is modal and consumes key and paste events. `<Esc>` blurs the
overlay unless it has an explicit keymap binding. Overlay-local keymaps use the
normal urvim key syntax and command strings:

```text
urvim.ui.overlays.set_keymap(overlay_id, "q", "plugin my-plugin close")
urvim.ui.overlays.set_keymap(overlay_id, "r", "write")
```

Bindings may invoke ordinary editor commands or commands registered by the
owning plugin. While an overlay is focused, its local mappings take precedence,
then global normal-mode mappings for focus and application commands are
inherited. Editor-only and unmapped keys are consumed rather than forwarded to
the last editor pane. A plugin can only access overlays it owns. Hiding,
blurring, or closing the focused overlay returns focus to the editor. Plugin
overlays are not saved in sessions.

### Split Plugin Panes

`urvim.panes` creates retained plugin UI as a first-class split-tree pane.
The first argument to `create` is a visible editor or plugin pane id; pass
`null` to split the currently focused pane. The existing pane remains the
first split child and the new plugin pane is the second child and receives
focus.

```text
let pane_id = urvim.panes.create(null, {
    "axis": "vertical",
    "ratio": { "first": 2, "second": 1 },
    "title": "My Plugin",
    "body_style": "ui.window",
    "header_style": "ui.tab.inactive",
    "focused_header_style": "ui.tab.active"
})

urvim.panes.set_content(pane_id, [
    [{ "text": "hello", "style": "syntax.keyword" }]
])
```

`axis` is `vertical` for side-by-side panes or `horizontal` for stacked
panes. The optional ratio defaults to `1:1`. Pane content fills the assigned
leaf and is clipped to the available content area. Pane ids use the same
numeric identity as editor pane ids, so any visible pane can be used as a
target. `urvim.panes.list()` returns every pane descriptor; plugin-only configuration, content, and keymap operations enforce ownership.

Plugin panes support the same retained styled content and local keymaps as
plugin overlays. They participate in normal focus, resize, equalize,
and close operations, but cannot be hidden and are not saved in sessions.
Closing a pane collapses its parent split. Plugin teardown closes all panes
owned by that plugin.

Every plugin pane reserves its first row for a header, including panes without a
title. The optional title is centered and clipped to the available width, and
retained content starts on the next row. The header uses `focused_header_style`
while focused and `header_style` while unfocused. They default to
`ui.tab.active` and `ui.tab.inactive`, respectively. `body_style` controls the
content area. Unlike plugin overlays, plugin panes do not accept
`border_style` because they use the layout's split separators instead of
drawing a separate border.

Plugin panes use the same keymap precedence and inheritance behavior as focused
plugin overlays.

### Line Formatting

`urvim.ui.line_format.render` exposes urvim's reusable line formatter to
plugins. It formats one line and returns the same nested content value accepted
by `urvim.ui.overlays.set_content`:

```text
let content = urvim.ui.line_format.render({
    "width": 40,
    "values": ["Name", "src/a-very-long-file-name.rs"],
    "sections": [
        {
            "style": "ui.window",
            "width": { "type": "fixed", "value": 12 },
            "alignment": "left"
        },
        {
            "style": null,
            "width": { "type": "flex", "weight": 1 },
            "overflow": {
                "type": "ellipsis",
                "placement": "end"
            }
        }
    ]
})
urvim.ui.overlays.set_content(overlay_id, content)
```

`width` is the available content width for this one-shot render. Each section
requires a width policy: `fixed` uses `value`, `measured` uses the value's
display width, and `flex` shares remaining width by `weight`. Sections default
to left alignment and clipping. Styles are optional theme tag names; `null` or
an omitted style uses the overlay body style. Formatting does not automatically
repeat after a terminal resize, so plugins should render again when they need
different width allocations.

Filesystem read/write callbacks are delivered later through the normal plugin dispatcher, not from filesystem worker threads. `urvim.fs.read_file`, `urvim.fs.write_file`, and `urvim.fs.read_dir` return a numeric request id immediately and call the callback once with a result payload.

Success payloads include `id`, `path`, and `ok = true`. `read_file` success also includes `text`; `read_dir` success also includes `entries`. Failure payloads include `id`, `path`, `ok = false`, and `error`.

Directory entries use this shape:

```text
{
    "path": "/path/to/file.rs",
    "name": "file.rs",
    "kind": "file" | "dir" | "symlink" | "other"
}
```

Job callbacks are delivered later through the normal plugin dispatcher, not from process I/O threads. Output callbacks receive text chunks and are not guaranteed to receive complete lines. `urvim.jobs.spawn` accepts `cmd`, `args`, `cwd`, `env`, `stdin`, `timeout_ms`, `on_stdout`, `on_stderr`, and `on_exit`.

Timer callbacks are also delivered later through the normal plugin dispatcher. Use `urvim.timers.defer(callback)` to run after the current callback returns, `urvim.timers.set_timeout(ms, callback)` to run once after a delay, and `urvim.timers.set_interval(ms, callback)` to run repeatedly. `urvim.timers.clear(timer_id)` cancels a timeout or interval that has not yet dispatched.

## Syntax Providers

Syntax providers run synchronously on the main editor thread for now. Providers should stay fast and return structured spans for the buffer snapshot they receive.

```text
fn init() {
    urvim.filetypes.register("simplelang")
    urvim.filetypes.detect_extension(".simple", "simplelang")
    urvim.syntax.register("simplelang", highlight_simplelang)
}

fn highlight_simplelang(snapshot) {
    return [{
        "range": {
            "start": { "row": 0, "col": 0 },
            "end": { "row": 0, "col": 2 }
        },
        "tag": "syntax.keyword"
    }]
}
```

Snapshot fields are `buffer_id`, `generation`, `filetype`, `path`, `text`, `lines`, `visible_range`, and `changed_range`. `lines` is a list of line strings without trailing newlines. `visible_range` is either `null` or `{ "start_row": n, "end_row": n }`.

Span ranges are 0-based byte offsets. A span may cross lines; urvim splits multiline spans into line-local cached spans internally. Tags should use names returned by `urvim.syntax.tags()` or compatible `syntax.*` theme tags.

## Event Hooks

Currently emitted event names:

- `EditorStarted`
- `EditorWillShutdown`
- `ThemeChanged`
- `BufferOpened`
- `BufferLoaded`
- `BufferSaved`
- `BufferSaveFailed`
- `BufferOpenFailed`
- `BufferClosed`
- `BufferUnloaded`
- `BufferPathChanged`
- `BufferReloaded`
- `ExternalFileConflict`
- `BufferChanged`
- `InsertBufferChanged`
- `InsertSessionChanged`
- `BufferModifiedChanged`
- `BufferFiletypeChanged`
- `PaneCreated`
- `PaneClosed`
- `PaneFocused`
- `TabOpened`
- `TabClosed`
- `TabActivated`
- `ActiveBufferChanged`
- `ModeChanged`
- `CursorMoved`
- `SelectionChanged`
- `LspServerStarted`
- `LspServerStartFailed`
- `LspServerStopped`
- `LspBufferAttached`
- `LspBufferDetached`
- `CommandExecuted`
- `DiagnosticsChanged`

`register_event_hook` returns a numeric hook id that can be passed to `unregister_event_hook`. Hooks are best-effort and run synchronously in the editor loop. They should stay quick.

All event payloads are maps with an `event` field containing the event name.

The buffer lifecycle events `BufferOpened`, `BufferLoaded`, `BufferSaved`, `BufferPathChanged`, `BufferReloaded`, `ExternalFileConflict`, `BufferClosed`, `BufferUnloaded`, and `BufferFiletypeChanged` also contain these fields, captured when the event is enqueued:

- `buffer_id`: numeric buffer id
- `path`: absolute path string, or `null` for an unnamed buffer
- `file_name`: path file name, or `null` for an unnamed buffer
- `filetype`: canonical filetype string
- `modified`: whether the buffer was modified

`BufferOpened` means a buffer changed from having zero visible tabs across the entire layout to having at least one. `BufferClosed` means its final visible tab was closed. Opening or closing another tab for a buffer that remains visible does not emit either event.

`BufferSaveFailed` and `BufferOpenFailed` contain `buffer_id` (or `null` when no buffer was identified), `path` (the attempted absolute path when resolution succeeded, otherwise `null`), `error_kind`, and `error`. `error_kind` is a stable lowercase category such as `not_found`, `permission_denied`, or `invalid_input`; `error` is the human-readable error message.

`BufferPathChanged` additionally contains `previous_path`, which is `null` when the buffer was previously unnamed. Save-as emits `BufferPathChanged` before `BufferSaved`; failed save-as operations do not change the path. `BufferReloaded` is emitted after a clean buffer is successfully reloaded from disk. `ExternalFileConflict` is emitted once for each distinct changed disk state observed while the buffer has unsaved edits, rather than on every filesystem poll.

Pane and tab ids are stable for the current editor process. Tab ids are newly allocated when session state is restored rather than persisted. Pane lifecycle payloads are:

- `PaneCreated`: `pane_id`, `kind`, and editor-only `buffer_id` and `tab_id`
- `PaneClosed`: `pane_id`, `kind`, and editor-only `buffer_id` and `tab_id`
- `PaneFocused`: `previous_pane_id`, `pane_id`, `kind`, and editor-only `buffer_id` and `tab_id`

Tab lifecycle payloads are:

- `TabOpened`: `pane_id`, `tab_id`, and the buffer snapshot fields listed above
- `TabClosed`: `pane_id`, `tab_id`, and the buffer snapshot fields captured before removal
- `TabActivated`: `previous_tab_id` (`null` for a newly created pane), `tab_id`, `pane_id`, and `buffer_id`

`ActiveBufferChanged` contains `previous_buffer_id` (`null` when absent), `buffer_id`, `pane_id`, and `tab_id`.

High-frequency events are coalesced independently for each buffer or pane over one completed terminal key, paste, maintenance/tick batch, plugin callback, or LSP effect. `BufferChanged` contains `buffer_id` and `changed_range`, but excludes edits owned by an active Insert or Replace session. `InsertBufferChanged` reports those edits granularly and contains `buffer_id` and `changed_range`. The range is the UTF-8-safe minimal replacement `{ start, old_end, new_end }`. Each position is `{ row, col }`, with zero-based rows and zero-based UTF-8 byte columns. `BufferModifiedChanged` contains `buffer_id`, `previous_modified`, and `modified`.

`InsertSessionChanged` is an aggregate notification emitted when an Insert or Replace session exits or its pane, tab, or buffer focus changes. Replace mode counts as an insert session. Initiating edits from commands such as `c`, `cc`, `o`, and `O` belong to the session. Its payload contains `pane_id`, `tab_id`, `buffer_id`, `mode`, and one `changed_range` covering the session's net text change. Plugin, LSP, reload, and maintenance edits remain ordinary `BufferChanged` events even while an insert session is active.

`ModeChanged` contains `pane_id`, `buffer_id`, `previous_mode`, and `mode`. `CursorMoved` contains `pane_id`, `buffer_id`, `previous_position`, and `position`. `SelectionChanged` contains `pane_id`, `buffer_id`, `previous_selection`, and `selection`; either selection may be `null`, otherwise it is `{ anchor, cursor, linewise }` using the same byte-column positions. Net no-ops do not emit an event, and changes to text, modified state, mode, cursor, and selection are separate events.

`BufferChanged`, `InsertBufferChanged`, `BufferModifiedChanged`, `ModeChanged`, `CursorMoved`, and `SelectionChanged` also contain `source: { kind, name }`. Stable kinds are `user`, `paste`, `undo`, `redo`, `plugin`, `lsp`, `reload`, and `internal`. `name` is the plugin ID for `plugin`, an optional server name for `lsp`, and `null` otherwise. An event directly produced by plugin P is delivered to other plugins but not back to P. This suppression applies only to the direct origin: events produced later by another hook are not treated as descendants of P.

Event draining captures one FIFO wave at a time, so events generated by hooks wait for the next dispatch wave and cannot overtake already queued lifecycle events. A causal chain is limited to 32 consecutive generated waves. On wave 33 urvim logs and displays a health warning, then drops the remaining generated chain to prevent unbounded hook processing.

Events produced by one layout transition are enqueued in this order: closed tabs, globally closed buffers, closed panes, created panes, opened tabs, globally opened buffers, focused pane, activated tabs, then active buffer change. Items within a category follow layout and tab order. Overlay lifecycle does not produce pane events. A split emits `PaneCreated`, editor-only tab and buffer events, and `PaneFocused`.

Fresh and session-restored layouts emit their initial state as one transition from an empty layout. Initial `PaneCreated`, `TabOpened`, `BufferOpened`, `PaneFocused`, `TabActivated`, and `ActiveBufferChanged` events are queued in the order above before `EditorStarted`.

`ThemeChanged` is emitted only when the committed theme name changes. Its payload contains `previous_theme`, `theme`, and `source`; stable source values are `picker`, `plugin`, and `fallback`. Picker highlight previews and cancel rollback are transient and do not emit events. Startup establishes the initial theme without emitting this event.

`EditorWillShutdown` is an observational, non-cancellable final notification. Its payload contains `reason` (`quit`, `last_pane_closed`, or `error`), sorted `modified_buffer_ids`, and `session_enabled`. Rejected `TryQuit` commands do not emit it. Once shutdown begins, urvim emits it exactly once and drains all hook-generated editor-event waves with the normal 32-wave protection while plugin APIs and the LSP runtime remain available. It then shuts down LSP and drains the resulting `LspBufferDetached`, diagnostics, and `LspServerStopped` events while plugin callbacks and editor APIs remain available. Session saving and terminal teardown happen only after those callbacks finish; commands issued by shutdown hooks cannot cancel this sequence.

An LSP session is identified by `server_name` plus `workspace_root`. `LspServerStarted` contains those fields and is emitted only after initialization succeeds. `LspServerStartFailed` additionally contains `error`; one event is emitted for a failed live session state even if document synchronization repeats. Removing all matching documents clears that failed state, so a later matching document can produce a new attempt and failure event. `LspServerStopped` additionally contains `reason`, currently `shutdown`.

`LspBufferAttached` contains the session identity plus `buffer_id`, `uri`, and `language_id`, and is emitted only after `didOpen` succeeds and the attachment is recorded. `LspBufferDetached` contains the same fields plus `reason` (`no_longer_eligible`, `file_deleted`, or `shutdown`) and is emitted only when an existing attachment is removed. Final shutdown emits all detach events for a session before its stop event. urvim does not currently emit lifecycle events for unexpected process crashes or supervise server restarts.

`DiagnosticsChanged` is a compact post-mutation snapshot. It contains `buffer_id`, `source` (an LSP server name or plugin diagnostics namespace), `cleared`, `source_count`, `total_count`, `errors`, `warnings`, `information`, and `hints`. Counts are captured after the source replacement or clear; severity counts aggregate all sources for that buffer. The full diagnostics are deliberately omitted and remain available through `urvim.diagnostics.get`. Replacing a source with identical diagnostics and clearing an absent source are no-ops and do not emit this event.

`CommandExecuted` contains `command`, a stable dotted and kebab-case semantic identifier, `success`, and `error` (`null` on success). Examples include `buffer.save`, `buffer.save-as`, `buffer.close`, `tab.next`, and `lsp.hover`. Plugin commands include their identifiers as `plugin.<plugin>.<command>`, preserving the plugin and command text. Accepted asynchronous commands report success immediately. Buffer domain events are queued before the command completion event.

Internal notification enqueueing, completion application, overwrite and git-discard confirmation results, and plugin picker, input, or confirmation responses do not emit `CommandExecuted`. `EditorStarted` has no additional fields.

`urvim.events` may expose reserved event-name constants for later plugin phases. Reserved events are not emitted and are not supported for hooks until listed above.

## Status

Use `plugin status` to show a compact runtime health summary with loaded plugin count, failed plugin count, total callbacks, slow callback count, and the slowest recorded callback duration.

## Themes

Plugin themes use the same TOML theme schema as built-in themes. Theme files are auto-discovered from direct `.toml` children of the plugin `themes/` directory. The theme file itself supplies the theme name.

Plugins can also manage themes from BearScript:

```text
fn init() {
    let name = urvim.themes.register("/absolute/path/to/theme.toml")
    urvim.themes.set(name)
}
```

`urvim.themes.register(path)` loads a TOML theme file immediately, inserts it into the theme registry, records ownership for the current plugin, and returns the resolved theme name. Duplicate theme names are rejected, including duplicates of built-in themes and auto-discovered plugin themes.

`urvim.themes.create(theme)` creates a theme directly from BearScript data using the same schema as TOML themes:

```text
fn init() {
    urvim.themes.create({
        "name": "Generated Dawn",
        "palette": {
            "bg": "#101010",
            "fg": "#eeeeee",
            "accent": "#7aa2f7",
            "muted": 244
        },
        "default": {
            "fg": "fg",
            "bg": "bg"
        },
        "highlights": {
            "ui.status_bar": {
                "fg": "bg",
                "bg": "accent",
                "bold": true
            },
            "syntax.comment": {
                "fg": "muted",
                "italic": true
            }
        }
    })
}
```

Palette values are either `"#rrggbb"` true-color strings or ANSI numbers from `0` to `255`. Style fields such as `fg`, `bg`, and `underline_color` reference palette names, matching the TOML schema. Inline colors inside styles are not supported.

Themes created from `init()` are available when startup selects `theme` from `config.toml`. See `examples/plugins/generated-theme` for a plugin that creates a generated theme and exposes `plugin generated-theme apply` to activate it.

`urvim.themes.unregister(name)` removes only dynamically registered themes owned by the current plugin. Auto-discovered `themes/*.toml` files and themes owned by other plugins cannot be removed this way.

`urvim.themes.list()` returns entries shaped like:

```text
{ "name": "Nord", "active": true }
```

## Security

Plugins are local files loaded at startup. Treat plugin directories as trusted input.
