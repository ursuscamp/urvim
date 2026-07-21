# Ghost Party

Ghost Party is a playful demonstration of urvim's plugin-owned ghost-text API. It summons styled inline ghosts, moves and restyles them on a timer, lists them, removes individual markers, and clears the party without affecting ghost text owned by other plugins.

## Enable the plugin

Add the example to `config.toml`, replacing the path with the checkout location:

```toml
[plugins.ghost-party]
path = "/path/to/urvim/examples/plugins/ghost-party"
```

## Commands

| Command                                 | Behavior                                             |
| --------------------------------------- | ---------------------------------------------------- |
| `plugin ghost-party summon`             | Add colorful ghosts to up to four non-empty lines.   |
| `plugin ghost-party dance`              | Start an opt-in animation at a modest interval.      |
| `plugin ghost-party freeze`             | Stop animation while leaving the ghosts visible.     |
| `plugin ghost-party inspect`            | Show the plugin's current ghost-text descriptors.    |
| `plugin ghost-party banish <marker-id>` | Remove one owned ghost.                              |
| `plugin ghost-party exorcise`           | Stop animation and clear all ghosts owned by plugin. |

The dance never starts automatically. Editing the buffer while ghosts are visible also demonstrates marker gravity: point anchors follow their surrounding text without changing the buffer itself.

## API demonstrated

The example exercises:

- `urvim.buffers.ghost_text.add`
- `urvim.buffers.ghost_text.update`
- `urvim.buffers.ghost_text.list`
- `urvim.buffers.ghost_text.remove`
- `urvim.buffers.ghost_text.clear`

Styles are partial overlays on the active theme's normal ghost-text style. Ghost Party supplies RGB foreground colors and boolean attributes while leaving the remaining properties theme-controlled.
