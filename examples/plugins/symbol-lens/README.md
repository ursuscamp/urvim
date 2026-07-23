# symbol-lens

A BearScript plugin demonstrating asynchronous LSP requests and cancellation.

## Commands

- `plugin symbol-lens hover_lens`: shows hover text at the active cursor.
- `plugin symbol-lens definition_preview`: shows the first definition target.
- `plugin symbol-lens completion_lens`: displays completion candidates.
- `plugin symbol-lens cancel_demo`: starts and immediately cancels a completion request.

The commands require an LSP server attached to the active buffer. Request
failures, timeouts, and cancellation are reported through the same callback
payload.

## Config

```toml
[plugins.symbol-lens]
enabled = true
path = "/path/to/urvim/examples/plugins/symbol-lens"
```

Or symlink the directory into the default plugin directory:

```sh
mkdir -p ~/.config/urvim/plugins
ln -s /path/to/urvim/examples/plugins/symbol-lens ~/.config/urvim/plugins/symbol-lens
```
