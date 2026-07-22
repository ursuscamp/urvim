# Funky Text

Funky Text demonstrates urvim's plugin-owned range-highlight API by finding every case-sensitive `FUNKY` substring in the active buffer and cycling its colors and text attributes.

## Enable the plugin

Add the example to `config.toml`, replacing the path with the checkout location:

```toml
[plugins.funky-text]
path = "/path/to/urvim/examples/plugins/funky-text"
```

## Try it

Open a buffer containing text such as:

```text
This text is FUNKY.
FUNKY colors make a FUNKY demo.
```

Then run:

| Command                   | Behavior                                                     |
| ------------------------- | ------------------------------------------------------------ |
| `plugin funky-text start` | Find each `FUNKY` match and start animating its style.       |
| `plugin funky-text stop`  | Stop animation while leaving the current highlights visible. |
| `plugin funky-text clear` | Stop animation and remove the plugin's highlights.           |

`start` binds the animation to the buffer that is active when the command runs. Running it again clears the previous run and rescans the current active buffer.

## API demonstrated

The example exercises:

- `urvim.buffers.highlights.add`
- `urvim.buffers.highlights.update`
- `urvim.buffers.highlights.list`
- `urvim.buffers.highlights.clear`
- `urvim.timers.set_interval`
- `urvim.timers.clear`

The highlights use half-open byte ranges around each five-byte `FUNKY` match. Their styles rotate through foreground and background colors plus bold, italic, underline, and reverse attributes.
