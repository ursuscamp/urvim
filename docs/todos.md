# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- build-time compiled themes
- show code action modification in preview window
- code action hints for the current line (with ghost text)
- quick jump
- healthcheck
- expand intents to have more scriptable actions

# Bugs

# Refactors

# Plugins

- can plugins expose APIs to each other?
- we need to look for more opportunities for events for plugins to react to
- ghost text access from plugins

- can we clarify/unify nomenclature of panes, windowgroups, windows across code and plugin
- failed test:

```
---- plugin::tests::buffers_module_errors_for_missing_buffer_and_out_of_range_row stdout ----

thread 'plugin::tests::buffers_module_errors_for_missing_buffer_and_out_of_range_row' (11758527) panicked at crates/urvim/src/plugin/mod.rs:2806:9:
assertion failed: out_of_range.contains("row 3 is out of range")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    plugin::tests::buffers_module_errors_for_missing_buffer_and_out_of_range_row
```
- can all text entered in insert mode be a single transaction?
