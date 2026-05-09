# urvim Configuration

This document describes the user-facing startup config file for urvim. It mirrors the canonical config schema used by the codebase.

## File Location

urvim reads an optional TOML config file from the XDG config directories:

- `$XDG_CONFIG_HOME/urvim/config.toml`
- `$XDG_CONFIG_DIRS/urvim/config.toml`, in order, using the first file found

If no config file exists, urvim starts with its built-in defaults.

## Precedence

Startup settings are resolved in this order:

1. Built-in defaults
2. TOML config file
3. Command-line flags

Command-line flags override config file values.

## Current Schema

The canonical config values are `theme`, `insert_escape`, `default_registers`, `syntax`, `todo_markers`, `auto_close_pairs`, `active_line`, `relative_number`, `indent_guides`, `auto_indent`, `advanced_glyphs`, `tab_insertion`, `tab_behavior`, `tab_width`, `scroll_margin`, `wrap_mode`, and `lsp`.

```toml
theme = "Friday Night"
insert_escape = "jk"
default_registers = { yank = "y", delete = "d", change = "c" }
syntax = true
todo_markers = ["TODO", "FIXME", "BUG", "NOTE"]
auto_close_pairs = true
active_line = false
indent_guides = true
auto_indent = "off"
advanced_glyphs = ["nerdfont"]
tab_insertion = "spaces"
tab_behavior = "simple"
tab_width = 4
scroll_margin = { vertical = 5, horizontal = 5 }
wrap_mode = "hard"

[lsp.rust_analyzer]
enabled = true
command = "rust-analyzer"
filetypes = ["rust"]
root_markers = ["Cargo.toml", "rust-project.json", ".git"]
```

### `theme`

Sets the active editor theme by name.

- Type: string
- Default: existing built-in default theme
- Override: `--theme <name>`
- Built-in themes: Friday Night, Saturday Morning, Rose Pine, Dracula, Tokyo Night, Catppuccin, Nord, OneDark, Gruvbox, and Gruvbox Light

### `insert_escape`

Sets an optional alternate insert-mode escape binding using urvim's canonical key string format.

- Type: string
- Default: not set
- Behavior: adds an additional insert-mode binding alongside `<Esc>`
- Examples: `jk`, `<C-[>`
- Validation: empty, whitespace-only, or malformed key strings are rejected at startup

### `default_registers`

Sets the default register selector used for yank, delete, and change operations.

- Type: inline TOML table with `yank`, `delete`, and `change` entries
- Default: `{ yank = "y", delete = "d", change = "c" }`
- Behavior: these values control which register selector is used when the corresponding operator runs without an explicit register prefix, and they define the destinations selected by `"` + `y`, `"` + `d`, and `"` + `c`
- Validation: each entry must be a single lowercase ASCII letter
- Scope: register selection for yank, delete, and change

### `syntax`

Controls whether syntax highlighting is enabled for rendered buffers.

- Type: boolean
- Default: `true`
- Override: `--no-syntax`
- Behavior: when `false`, buffers still detect filetypes and the status bar still shows the syntax label, but rendered text uses the base theme style only

### `todo_markers`

Sets the list of standalone marker tokens that receive comment-scoped todo highlighting.

- Type: array of strings
- Default: `["TODO", "FIXME", "BUG", "NOTE"]`
- Behavior: the configured list replaces the built-in defaults entirely
- Matching: markers are matched case-sensitively and only when they appear as standalone words inside comment spans
- Styling: each marker is mapped to a marker-specific syntax tag such as `comment.todo`, `comment.fixme`, `comment.bug`, or `comment.note`
- Validation: entries must be non-empty standalone word tokens that normalize to valid theme tags
- Scope: rendered comments only

### `auto_close_pairs`

Controls whether insert mode automatically pairs supported brackets and quotes.

- Type: boolean
- Default: `true`
- Behavior: when `true`, insert mode auto-closes parentheses, square brackets, curly braces, double quotes, single quotes, and backticks; typing a supported closer next to an auto-inserted closer skips over it; pressing backspace between a supported opener and closer removes both characters
- Behavior when `false`: opening and closing brackets and quotes insert as plain text, and backspace deletes only one character at a time
- Scope: insert mode only

### `active_line`

Controls whether the focused window highlights the cursor line in normal mode.

- Type: boolean
- Default: `false`
- Behavior: when `true`, the current line in the focused window receives the theme's active-line UI style while the editor is in normal mode; when `false`, the editor keeps its current rendering behavior
- Scope: focused window only, normal mode only

### `relative_number`

Controls whether the gutter shows relative line numbers around the cursor line.

- Type: boolean
- Default: `false`
- Behavior: when `true`, the cursor line keeps its absolute buffer line number while surrounding visible lines show their distance from the cursor line; when `false`, the gutter shows absolute line numbers only
- Scope: all editor modes

### `indent_guides`

Controls whether the focused window renders the active indent scope guide.

- Type: boolean
- Default: `true`
- Behavior: when `true`, the editor renders a vertical guide for the active indent scope at the cursor's current visual indentation depth; when `false`, indent guides are not rendered
- Scope: focused window rendering

### `auto_indent`

Controls how insert mode chooses indentation when creating a new line.

- Type: string enum
- Default: `"off"`
- Supported values: `off`, `neighbor`
- Behavior: `off` preserves plain newline insertion; `neighbor` looks at nearby non-blank buffer lines and reuses the most-indented relevant leading whitespace prefix when creating a new line
- Extensibility: the setting is intentionally enum-based so additional auto-indent styles can be added later without changing the config field shape
- Scope: insert-mode newline creation and normal-mode open-line commands

### `advanced_glyphs`

Controls optional glyph rendering capabilities used by the editor UI.

- Type: array of strings
- Default: empty
- Supported values: `nerdfont`, `unicode`, `unicode_borders`, `unicode_indent`
- Behavior: when `nerdfont` is enabled, filetypes with glyph metadata can render icons in the tab bar and status bar; when it is not enabled, the UI stays text-only
- Behavior: when `unicode` is enabled, all Unicode glyph capabilities are enabled (`unicode_borders` and `unicode_indent`)
- Behavior: when `unicode_borders` is enabled, split borders render with Unicode box-drawing glyphs
- Behavior: when `unicode_indent` is enabled, indent guides render with Unicode line-drawing glyphs
- Validation: unknown capability names are rejected at startup

### `tab_insertion`

Sets how insert-mode `Tab` behaves when it uses the configured insertion style directly.

- Type: string
- Default: `"spaces"`
- Supported values: `tabs`, `spaces`
- Behavior: `tabs` inserts a literal tab character; `spaces` inserts a run of spaces sized to the configured `tab_width`

### `tab_behavior`

Sets whether insert-mode `Tab` uses the configured insertion style directly or infers a style from the active buffer.

- Type: string
- Default: `"simple"`
- Supported values: `simple`, `smart`
- Behavior: `simple` always uses `tab_insertion`; `smart` inspects existing indentation in the active buffer and falls back to `tab_insertion` when no clear style exists

### `tab_width`

Sets the visual width of tab characters in buffer rendering and the number of spaces inserted when `tab_insertion = "spaces"`.

- Type: positive integer
- Default: `4`
- Behavior: tab characters render as a fixed-width block using the configured width, so the visible tab expansion does not depend on the current cursor column; space-based tab insertion uses that same width

### `scroll_margin`

Sets the visual margin bands near viewport edges that trigger scrolling before edge crossing.

- Type: table with `vertical` and `horizontal` integer entries
- Default: `{ vertical = 5, horizontal = 5 }`
- Behavior: vertical scrolling starts once the cursor enters the top/bottom margin bands; horizontal scrolling starts once the cursor enters the left/right margin bands
- Small viewport behavior: effective margins automatically clamp down when the viewport is too small to satisfy configured values

### `wrap_mode`

Sets how long logical lines break when visual wrapping is enabled for a window.

- Type: string enum
- Default: `"hard"`
- Supported values: `hard`, `soft`
- Behavior: `hard` wraps at the exact content width; `soft` wraps at the nearest word boundary at or before the content width, and falls back to a hard split when no boundary exists
- Scope: wrapped window rendering only (wrapping is toggled per window)

### `lsp`

Controls built-in language server configurations.

- Type: nested TOML table
- Default: all servers disabled
- Built-in servers: `rust_analyzer`
- Behavior: builtin server definitions are loaded from statically included TOML files at startup; user overrides deep-merge into those built-in defaults, so you can enable a server by setting `enabled = true` and override individual server fields as needed
- `rust_analyzer` defaults: `command = "rust-analyzer"`, `filetypes = ["rust"]`, `root_markers = ["Cargo.toml", "rust-project.json", ".git"]`, `settings.workspace.symbol.search.kind = "all_symbols"`
- Scope: language-server startup and attachment only

## Sessions

urvim also saves and restores workspace sessions automatically when it is started with no file paths.

- Storage: `$XDG_DATA_HOME/urvim/sessions/`, with a fallback to `$HOME/.local/share/urvim/sessions/`
- Keying: sessions are matched to the raw current working directory
- Filename format: `<sanitized-folder-name>--<short-hash>.toml`
- Restore behavior: if a matching session exists, urvim restores the workspace automatically
- Fallback: if no session exists or a session cannot be read, urvim starts with a blank buffer
- Autosave: the current session is written every 10 seconds while the workspace state is dirty
- Scope: workspace state only; unsaved buffer contents are not persisted

## Notes

- The config file is TOML.
- Unknown fields are treated as configuration errors.
- Future config values will be added here as the schema grows.
