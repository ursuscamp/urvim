# Plugins

urvim supports explicitly enabled local plugins. A plugin directory contains `urvim-plugin.toml` plus any files referenced by that manifest.

Plugins can contribute static themes and command scripts. They can also start a local process and expose namespaced process commands.

For process protocol details, see `docs/plugin-protocol.md`.

## Loading Model

Plugins are never auto-discovered. A plugin is loaded only when it appears in `config.toml` under `[plugins.<plugin-id>]`.

Default path:

```toml
[plugins.demo-plugin]
enabled = true
```

When `path` is omitted, urvim loads the plugin from `$XDG_CONFIG_HOME/urvim/plugins/<plugin-id>`.

Explicit path:

```toml
[plugins.demo-plugin]
enabled = true
path = "~/src/urvim/plugins/demo-plugin"
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
name = "demo-plugin"
version = "0.1.0"
description = "Example manifest-only urvim plugin"

themes = ["themes/demo-night.toml"]

[process]
command = "uv"
args = ["run", "python", "-m", "demo_plugin"]
env = { RUST_LOG = "info" }

[commands.echo]
description = "Echo text through the demo plugin process."
request = "demo/echo"

[scripts]
wq = ["write", "quit"]
save_as_rust = ["buffer filetype filetype=rust", "buffer write path={1}"]
rename_write = ["lsp rename name={name}", "write"]
```

Fields:

- `name`: required plugin namespace; must match `[plugins.<plugin-id>]`.
- `version`: required non-empty version string.
- `description`: optional human-readable text.
- `themes`: optional array of relative theme file paths.
- `process`: optional process plugin launch table.
- `commands`: optional table of process-backed commands.
- `scripts`: optional table from script name to ordered command strings.

Validation:

- Plugin names and script names must be non-empty.
- Plugin names and script names must not contain whitespace or path separators.
- Process command names follow script-name rules and must not conflict with script names in the same plugin.
- Process command `request` values must be non-empty and contain no whitespace.
- Theme paths must be relative and must stay inside the plugin directory.
- Unknown manifest fields are rejected.

## Themes

Plugin themes use the same TOML theme schema as built-in themes. The manifest lists only file paths; the theme file itself supplies the theme name.

Example:

```toml
themes = ["themes/demo-night.toml"]
```

Then in `themes/demo-night.toml`:

```toml
name = "Demo Night"

[palette]
bg = "#10141f"
fg = "#d7e1f0"

[default]
fg = "fg"
bg = "bg"
```

To select a plugin theme:

```toml
theme = "Demo Night"
```

Theme loading behavior:

- Built-in themes load first.
- Plugin themes load after built-ins.
- Duplicate theme names are rejected, including duplicates of built-in themes.
- Invalid plugin theme TOML fails startup with plugin id and path context.

## Scripts

Plugin scripts use the same command strings and placeholder syntax as configured user scripts.

Example manifest scripts:

```toml
[scripts]
wq = ["write", "quit"]
save_as_rust = ["buffer filetype filetype=rust", "buffer write path={1}"]
rename_write = ["lsp rename name={name}", "write"]
```

Run them through the plugin namespace:

```text
plugin demo-plugin wq
plugin demo-plugin save_as_rust src/main.rs
plugin demo-plugin rename_write name=new_symbol
```

Placeholder rules:

- `{1}`, `{2}`, and so on reference positional arguments after the script name.
- `{name}` references named arguments such as `name=value`.
- Missing placeholders are reported before any script command runs.

Plugin scripts do not create top-level command roots. A plugin script named `wq` does not conflict with a user-configured `[scripts].wq`.

## Process Plugins

Process-backed plugins start during normal editor startup when a loaded manifest contains `[process]`. Startup failures are logged and reported as warnings, but static manifest contributions remain available.

Manifest process table:

```toml
[process]
command = "uv"
args = ["run", "python", "-m", "demo_plugin"]
env = { RUST_LOG = "info" }
```

Protocol foundation:

- Transport: plugin process stdio.
- Framing: `u32` big-endian payload length followed by MessagePack bytes.
- Encoding: MessagePack through `rmp-serde`.
- Message envelope: request, response, and notification.
- Protocol version: `1`.
- The editor sends `protocol_version`, editor metadata, plugin metadata, and editor capabilities in `editor/initialize`.
- Plugins must respond to `editor/initialize` with matching `protocol_version` and a `capabilities` array.
- Process commands only run when the plugin advertised the command request method as a capability.
- Static manifest contributions remain available even when a process cannot be started.
- Full request, notification, editor API, and error conventions are documented in `docs/plugin-protocol.md`.

Initialize response example:

```json
{
  "protocol_version": 1,
  "capabilities": ["demo/echo"]
}
```

Process commands:

```toml
[commands.echo]
description = "Echo text through the demo plugin process."
request = "demo/echo"
```

Run them through the plugin namespace:

```text
plugin demo-plugin echo text=hello
```

Named arguments become object fields in the request params. Positional arguments are passed as an `args` array.

Plugin runtime status:

```text
plugin status
```

This reports loaded plugin process states, failed process errors, and advertised capabilities.

## Example Plugin

The living example plugin is in `examples/plugins/demo-plugin`.

It demonstrates:

- A plugin manifest.
- A plugin-provided theme named `Demo Night`.
- A multi-command script.
- A positional-placeholder script.
- A named-placeholder script.
- A Python process plugin.
- A process command named `echo`.
- Editor API requests from the plugin process.
- Optional buffer mutation through `plugin demo-plugin echo insert=...`.

For local testing with the default plugin path:

```sh
mkdir -p ~/.config/urvim/plugins
ln -s /Users/ryan/Dev/urvim/examples/plugins/demo-plugin ~/.config/urvim/plugins/demo-plugin
```

Then add this to config:

```toml
[plugins.demo-plugin]
enabled = true
```

## Security

Plugins are local files loaded at startup. Treat plugin directories as trusted input.

Plugin scripts execute editor commands. Process plugins execute arbitrary local processes, so keep plugin loading explicit and only enable plugins you trust.
