# Demo Plugin

This plugin is the living example for urvim's plugin system. It is intended for plugin authors and for local development testing as the plugin system grows.

## Features

- Provides the `Demo Night` theme from `themes/demo-night.toml`.
- Provides `wq`, a multi-command script.
- Provides `save_as_rust`, a script using positional placeholder `{1}`.
- Provides `rename_write`, a script using named placeholder `{name}`.
- Includes a Python process plugin managed by `uv` for protocol/runtime testing.
- Provides `echo`, a process command that sends `demo/echo` to the Python plugin.
- Demonstrates editor API requests from the plugin process.
- Can optionally insert text into the active buffer through `editor/applyEdit`.

## Config

Default plugin directory setup:

```toml
[plugins.demo-plugin]
enabled = true
```

With the default config, urvim loads this plugin from `$XDG_CONFIG_HOME/urvim/plugins/demo-plugin`.

Explicit path setup:

```toml
[plugins.demo-plugin]
enabled = true
path = "/Users/ryan/Dev/urvim/examples/plugins/demo-plugin"
```

To use the example theme:

```toml
theme = "Demo Night"
```

## Scripts

These scripts are invoked through the plugin namespace:

```text
plugin demo-plugin wq
plugin demo-plugin save_as_rust src/main.rs
plugin demo-plugin rename_write name=new_symbol
```

## Process

The process plugin is a small Python package managed by `uv`.

```sh
uv run python -m demo_plugin
```

The manifest starts it the same way:

```toml
[process]
command = "uv"
args = ["run", "python", "-m", "demo_plugin"]
```

On startup the Python process responds to `editor/initialize` with protocol version `1` and advertises `demo/echo` as its process command capability.

Process command example:

```text
plugin demo-plugin echo text=hello
```

The command asks urvim for `editor/getActiveBuffer` and includes that metadata in its response.

Mutation example:

```text
plugin demo-plugin echo insert="hello from plugin"
```

When `insert` is provided, the plugin sends `editor/applyEdit` to insert the text at the active buffer cursor.

Status example:

```text
plugin status
```

This reports the plugin process state and advertised capabilities.

## Protocol Reference

See `docs/plugin-protocol.md` for MessagePack framing, initialize handshake, editor requests, notifications, and error conventions.

## Local Symlink

For local development, symlink this directory into the default plugin location:

```sh
mkdir -p ~/.config/urvim/plugins
ln -s /Users/ryan/Dev/urvim/examples/plugins/demo-plugin ~/.config/urvim/plugins/demo-plugin
```
