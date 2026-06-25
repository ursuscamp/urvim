# Simple Syntax Plugin

This example registers a BearScript syntax provider for the custom `simplelang` filetype.

Enable it in `config.toml`:

```toml
[plugins.simple-syntax]
enabled = true
path = "/Users/ryan/Dev/urvim/examples/plugins/simple-syntax"
```

Open the example file:

```sh
cargo run -- examples/simplelang/hello.simple
```

Then run this command inside urvim:

```text
plugin simple-syntax simplelang
```

The plugin highlights keywords, strings, comments, uppercase constants, and integers.
