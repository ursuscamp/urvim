# API Host Plugin

This example exposes two versioned cross-plugin API endpoints:

- `greet.v1`, which returns a greeting and the caller's plugin id.
- `add.v1`, which returns the sum of two numbers and the caller's plugin id.

The companion `api-caller` plugin discovers and calls these endpoints.
