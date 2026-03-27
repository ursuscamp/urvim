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

The first canonical config value is `theme`.

```toml
theme = "Friday Night"
```

### `theme`

Sets the active editor theme by name.

- Type: string
- Default: existing built-in default theme
- Override: `--theme <name>`

## Notes

- The config file is TOML.
- Unknown fields are treated as configuration errors.
- Future config values will be added here as the schema grows.
