# window-demo

An example BearScript plugin that toggles the same retained UI between a
floating window and a split-pane plugin UI. It renders styled help text with
`urvim.ui.line_format` and binds keys to plugin commands.

Configure it with:

```toml
[plugins.window-demo]
enabled = true
path = "/Users/ryan/Dev/urvim/examples/plugins/window-demo"
```

The floating window opens and focuses during plugin initialization. Press
`d` to replace it with a docked vertical pane using a 2:1 editor-to-plugin
ratio. Press `d` again to return to the centered floating window. Run
`plugin window-demo open` to focus the current representation again.

When the demo window is focused:

- `h` moves it to the center.
- `j` moves it to the top-center with a larger top margin.
- `k` moves it to the top-center with a smaller top margin.
- `l` moves it to the top-right with top and right margins.
- `f` moves it to fixed coordinates near the top-left.
- `d` toggles between the floating window and docked split pane.
- `r` refreshes its retained content.
- `t` changes its title to demonstrate window reconfiguration.
- `q` closes it.
- `Esc` blurs it.

The floating-only movement commands are unavailable while docked. In pane
mode, normal pane focus and resize commands apply to the demo pane. The
command-line commands `plugin window-demo center`, `top_center`, `top_right`,
`bottom_right`, `fixed`, `higher`, `lower`, `show`, `focus`, `refresh`,
`close`, and `dock` exercise the same API outside the surface-local keymap.

This plugin is intended for manual testing of window rendering, clipping,
focus routing, resizing, ownership, margin updates, content updates, and
cleanup. Its formatted rows demonstrate fixed, measured, and flex sections,
alignment, ellipsis, theme tags, and body-style fallbacks. Margin updates
accept optional `top`, `right`, `bottom`, and `left` sides; `null` clears a
side.
