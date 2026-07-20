# API Host Plugin

This example exposes two versioned cross-plugin API endpoints:

- `greet.v1`, which returns a greeting and the caller's plugin id.
- `add.v1`, which returns the sum of two numbers and the caller's plugin id.

After handling either endpoint, the plugin also emits the custom event
`request.completed.v1`. Its value contains the endpoint name and caller plugin
id. The companion `api-caller` plugin discovers and calls the APIs while also
subscribing to this event.
