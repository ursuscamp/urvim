# API Caller Plugin

This example discovers and calls endpoints exposed by the companion `api-host` plugin.

Enable both plugins with paths pointing to this repository:

```toml
[plugins.api-host]
path = "/path/to/urvim/examples/plugins/api-host"

[plugins.api-caller]
path = "/path/to/urvim/examples/plugins/api-caller"
```

Available commands:

- `plugin api-caller demo` calls `greet.v1` and `add.v1` and displays their results.
- `plugin api-caller discover` lists the endpoints exposed by `api-host`.
- `plugin api-caller error` calls a missing endpoint and displays the structured error response.

The caller and host may appear in either configuration order because calls are resolved after plugin loading.
