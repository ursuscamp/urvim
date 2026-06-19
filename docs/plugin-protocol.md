# Plugin Protocol

Process plugins communicate with urvim over the plugin process standard input and output streams. The protocol is intended for local, trusted plugins and currently uses protocol version `1`.

See `examples/plugins/demo-plugin` for an executable Python example managed by `uv`.

## Transport

Every message is a MessagePack payload framed by a 4-byte unsigned big-endian length.

Frame layout:

```text
u32_be_length messagepack_payload
```

Rules:

- The length covers only the MessagePack payload, not the 4-byte header.
- Messages are encoded with string keys.
- The process must read from stdin and write responses or notifications to stdout.
- Plugin stderr is not part of the protocol.

## Message Envelope

Requests expect a response:

```json
{
  "type": "request",
  "id": 2,
  "method": "demo/echo",
  "params": { "text": "hello" }
}
```

Responses answer a request id:

```json
{
  "type": "response",
  "id": 2,
  "result": { "ok": true }
}
```

Error responses use the same envelope with `error` instead of `result`:

```json
{
  "type": "response",
  "id": 2,
  "error": "unknown buffer_id 999"
}
```

Notifications are fire-and-forget:

```json
{
  "type": "notification",
  "method": "editor/notify",
  "params": { "level": "info", "message": "ready" }
}
```

## Lifecycle

When a loaded plugin manifest contains `[process]`, urvim starts the configured process with the plugin root as the current working directory.

Startup sequence:

- urvim spawns the process.
- urvim sends `editor/initialize`.
- The plugin responds with matching protocol version and advertised capabilities.
- urvim starts the asynchronous reader and marks the process `Running`.
- Startup failures mark only that process as failed; manifest themes, scripts, and command declarations remain loaded.

Shutdown sequence:

- urvim kills and waits for running plugin children.
- Reader threads are joined where practical.

## Initialize

urvim sends:

```json
{
  "type": "request",
  "id": 1,
  "method": "editor/initialize",
  "params": {
    "protocol_version": 1,
    "editor": { "name": "urvim", "version": "0.1.0" },
    "plugin": { "name": "demo-plugin", "version": "0.1.0" },
    "capabilities": [
      "editor/notify",
      "editor/getActiveBuffer",
      "editor/getBufferText",
      "editor/getConfig",
      "editor/applyEdit"
    ]
  }
}
```

The plugin must respond:

```json
{
  "type": "response",
  "id": 1,
  "result": {
    "protocol_version": 1,
    "capabilities": ["demo/echo"]
  }
}
```

Policy:

- Only protocol version `1` is accepted.
- Missing or non-string capabilities fail initialization.
- Process commands declared in the manifest only run if the plugin advertised the command request method as a capability.

## Manifest Commands

Manifest process commands map a command name to a request method:

```toml
[commands.echo]
description = "Echo text through the demo plugin process."
request = "demo/echo"
```

The user command:

```text
plugin demo-plugin echo text=hello
```

sends:

```json
{
  "type": "request",
  "id": 2,
  "method": "demo/echo",
  "params": { "text": "hello" }
}
```

Named command arguments become object fields. Positional arguments are sent as an `args` array.

## Editor Notifications

Plugins can notify the editor with `editor/notify`:

```json
{
  "type": "notification",
  "method": "editor/notify",
  "params": {
    "level": "info",
    "message": "demo plugin initialized"
  }
}
```

Levels:

- `info`
- `warn` or `warning`
- `error`

Unknown levels are downgraded to warnings.

## Editor Requests

Plugins may send read-only requests and scoped edit requests to the editor. Editor requests must be sent as request envelopes from the plugin process. urvim responds on stdout with the same id.

### `editor/getActiveBuffer`

Params:

```json
{}
```

Result:

```json
{
  "id": 0,
  "path": "/tmp/demo.rs",
  "file_name": "demo.rs",
  "filetype": "rust",
  "line_count": 12,
  "modified": false,
  "cursor": { "line": 0, "col": 0 }
}
```

`path` and `file_name` are `null` for unnamed buffers.

### `editor/getBufferText`

Params:

```json
{ "buffer_id": 0 }
```

Result:

```json
{
  "buffer_id": 0,
  "text": "buffer contents"
}
```

Unknown buffers return an error response.

### `editor/getConfig`

Params:

```json
{}
```

Result is a safe subset of configuration:

```json
{
  "theme": "Friday Night",
  "syntax": true,
  "active_line": true,
  "relative_number": false,
  "indent_guides": true,
  "auto_close_pairs": true,
  "tab_width": 4,
  "plugins": ["demo-plugin"]
}
```

### `editor/applyEdit`

Params for insert:

```json
{
  "buffer_id": 0,
  "kind": "insert",
  "start": { "line": 0, "col": 0 },
  "text": "hello"
}
```

Params for delete:

```json
{
  "buffer_id": 0,
  "kind": "delete",
  "start": { "line": 0, "col": 0 },
  "end": { "line": 0, "col": 5 }
}
```

Params for replace:

```json
{
  "buffer_id": 0,
  "kind": "replace",
  "start": { "line": 0, "col": 0 },
  "end": { "line": 0, "col": 5 },
  "text": "hello"
}
```

Result:

```json
{
  "buffer_id": 0,
  "applied": true,
  "text": "new buffer contents"
}
```

Invalid buffer ids, invalid cursor ranges, missing `text`, and unsupported edit kinds return error responses and do not mutate the buffer.

## Error Conventions

Use response `error` for request failures:

```json
{
  "type": "response",
  "id": 42,
  "error": "explanation"
}
```

Conventions:

- Keep error strings stable enough for humans to understand.
- Use request ids exactly as received.
- Do not send both `result` and `error`.
- Initialization errors fail the process startup.
- Unknown editor request methods receive an error response.

## Python Framing Example

```python
import struct
import sys
import msgpack


def read_message():
    header = sys.stdin.buffer.read(4)
    if not header:
        return None
    (length,) = struct.unpack(">I", header)
    payload = sys.stdin.buffer.read(length)
    return msgpack.unpackb(payload, raw=False)


def write_message(message):
    payload = msgpack.packb(message, use_bin_type=True)
    sys.stdout.buffer.write(struct.pack(">I", len(payload)))
    sys.stdout.buffer.write(payload)
    sys.stdout.buffer.flush()
```

## Demo Plugin With `uv`

The demo plugin can be run directly from its directory:

```sh
uv run python -m demo_plugin
```

In normal editor use, enable it in `config.toml`:

```toml
[plugins.demo-plugin]
enabled = true
path = "/Users/ryan/Dev/urvim/examples/plugins/demo-plugin"
```

Then run:

```text
plugin demo-plugin echo text=hello
plugin demo-plugin echo insert="hello from plugin"
plugin status
```
