# find-demo

An example BearScript plugin that uses `urvim.search.find` to provide an
alternative Find interface. It prompts for a case-insensitive literal query,
then presents every match in a searchable picker with line context and
location. Selecting a result moves the editor cursor to that match.

Configure it with an absolute path to this example:

```toml
[plugins.find-demo]
enabled = true
path = "/absolute/path/to/urvim/examples/plugins/find-demo"
```

Open the Find input with:

```text
plugin find-demo open
```

A query can also be supplied directly:

```text
plugin find-demo open search terms
```

An optional normal-mode keymap can make it easier to open:

```toml
[keymaps.normal]
"<Space>/" = "plugin find-demo open"
```

After submitting the query, type in the result picker to filter line context,
use the arrow keys or Ctrl-P/Ctrl-N to move, and press Enter or Ctrl-Y to jump
to the selected match. Esc or Ctrl-C cancels without moving the cursor.
