# cargo-fmt

An example urvim plugin that runs `cargo fmt` on Rust files after they are saved.

## How it works

- Registers a `BufferSaved` event hook during initialization.
- On save, checks that the buffer filetype is `rust` and the path ends with `.rs`.
- Runs `cargo fmt -- <path>` from the editor process working directory.
- Shows a warning notification if formatting fails.

## Config

```toml
[plugins.cargo-fmt]
enabled = true
path = "/path/to/urvim/examples/plugins/cargo-fmt"
```

Or symlink into the default plugin directory:

```sh
mkdir -p ~/.config/urvim/plugins
ln -s /Users/ryan/Dev/urvim/examples/plugins/cargo-fmt ~/.config/urvim/plugins/cargo-fmt
```

## Requirements

- `cargo` on PATH
