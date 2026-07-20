# API Caller Plugin

This example discovers and calls endpoints exposed by the companion `api-host`
plugin. It also subscribes to the host's `request.completed.v1` custom event and
displays each broadcast.

Enable both plugins with paths pointing to this repository:

```toml
[plugins.api-host]
path = "/path/to/urvim/examples/plugins/api-host"

[plugins.api-caller]
path = "/path/to/urvim/examples/plugins/api-caller"
```

Available commands:

- `plugin api-caller demo` calls `greet.v1` and `add.v1`, displays their directed
  API responses, and receives the host's custom event broadcasts.
- `plugin api-caller discover` lists the endpoints exposed by `api-host`.
- `plugin api-caller error` calls a missing endpoint and displays the structured error response.

The caller and host may appear in either configuration order because API calls
and custom events are resolved after plugin loading.
