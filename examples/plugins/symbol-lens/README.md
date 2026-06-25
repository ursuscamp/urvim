# symbol-lens

An example urvim plugin that demonstrates async LSP-backed editor APIs.

## Commands

- `plugin symbol-lens hover_lens`: shows hover text at the active cursor.
- `plugin symbol-lens definition_preview`: shows the first definition target as `path:line:col`.
- `plugin symbol-lens completion_lens`: shows the completion count and top five candidate labels.

These commands require an attached LSP server for the active buffer. If no LSP result is available, the plugin reports a friendly message through `editor/notify`.

## Config

```toml
[plugins.symbol-lens]
enabled = true
path = "/path/to/urvim/examples/plugins/symbol-lens"
```

Or symlink into the default plugin directory:

```sh
mkdir -p ~/.config/urvim/plugins
ln -s /Users/ryan/Dev/urvim/examples/plugins/symbol-lens ~/.config/urvim/plugins/symbol-lens
```

## Requirements

- An LSP server configured for the active buffer's filetype.
- `uv` for running the Python plugin process.
