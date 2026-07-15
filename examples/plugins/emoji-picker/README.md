# emoji-picker

An example BearScript plugin that uses `urvim.ui.pickers` to search a curated
emoji list and insert the selected emoji at the active editor cursor.

Configure it with an absolute path to this example:

```toml
[plugins.emoji-picker]
enabled = true
path = "/absolute/path/to/urvim/examples/plugins/emoji-picker"
```

Open the picker with:

```text
plugin emoji-picker open
```

An optional normal-mode keymap can make it easier to open:

```toml
[keymaps.normal]
"<Space>e" = "plugin emoji-picker open"
```

Type to search emoji names and aliases. Use the arrow keys or Ctrl-P/Ctrl-N to
move, Enter or Ctrl-Y to select, Tab to switch between fuzzy and exact search,
and Esc or Ctrl-C to cancel. Selection inserts the emoji at the active cursor;
cancellation leaves the buffer unchanged.
