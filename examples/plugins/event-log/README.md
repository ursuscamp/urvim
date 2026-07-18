# event-log

An example urvim plugin that records editor events in `events.log`.

## How it works

- Subscribes to the complete currently emitted plugin event catalog.
- Serializes each event payload as one JSON object per line.
- Keeps one asynchronous file write in flight at a time so rapid events remain ordered.
- Starts a new log for each editor run.
- Writes `events.log` relative to the directory where urvim was started.

High-frequency events such as `BufferChanged` and `CursorMoved` can make the log grow quickly during normal editing.

## Config

```toml
[plugins.event-log]
enabled = true
path = "/path/to/urvim/examples/plugins/event-log"
```

Or symlink into the default plugin directory:

```sh
mkdir -p ~/.config/urvim/plugins
ln -s /Users/ryan/Dev/urvim/examples/plugins/event-log ~/.config/urvim/plugins/event-log
```
