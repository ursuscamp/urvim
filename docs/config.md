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

The canonical config values are `theme`, `insert_escape`, `syntax`, `auto_close_pairs`, and `advanced_glyphs`.

```toml
theme = "Friday Night"
insert_escape = "jk"
syntax = true
auto_close_pairs = true
advanced_glyphs = ["nerdfont"]
```

### `theme`

Sets the active editor theme by name.

- Type: string
- Default: existing built-in default theme
- Override: `--theme <name>`

### `insert_escape`

Sets an optional alternate insert-mode escape binding using urvim's canonical key string format.

- Type: string
- Default: not set
- Behavior: adds an additional insert-mode binding alongside `<Esc>`
- Examples: `jk`, `<C-[>`
- Validation: empty, whitespace-only, or malformed key strings are rejected at startup

### `syntax`

Controls whether syntax highlighting is enabled for rendered buffers.

- Type: boolean
- Default: `true`
- Override: `--no-syntax`
- Behavior: when `false`, buffers still detect filetypes and the status bar still shows the syntax label, but rendered text uses the base theme style only

### `auto_close_pairs`

Controls whether insert mode automatically pairs supported brackets and quotes.

- Type: boolean
- Default: `true`
- Behavior: when `true`, insert mode auto-closes parentheses, square brackets, curly braces, double quotes, single quotes, and backticks; typing a supported closer next to an auto-inserted closer skips over it; pressing backspace between a supported opener and closer removes both characters
- Behavior when `false`: opening and closing brackets and quotes insert as plain text, and backspace deletes only one character at a time
- Scope: insert mode only

### `advanced_glyphs`

Controls optional glyph rendering capabilities used by the editor UI.

- Type: array of strings
- Default: empty
- Supported values: `nerdfont`
- Behavior: when `nerdfont` is enabled, filetypes with glyph metadata can render icons in the tab bar and status bar; when it is not enabled, the UI stays text-only
- Validation: unknown capability names are rejected at startup

## Notes

- The config file is TOML.
- Unknown fields are treated as configuration errors.
- Future config values will be added here as the schema grows.
