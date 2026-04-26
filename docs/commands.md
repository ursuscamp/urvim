# Command Line

urvim supports a normal-mode command line opened with `:`.

## Behavior

- Opens in a bordered floating window that defaults to a 55-column width, capped by the available screen width, and keeps a fixed input area with horizontal scrolling once the text reaches the edge.
- `Enter` executes the current command.
- `Esc` closes the command line without executing.
- `Backspace` deletes the previous character.
- History navigation (session-only):
  - `Up` / `Down`
  - `Ctrl-p` / `Ctrl-n`
- Command line always closes after an execute attempt (success or error).

## Supported Commands

### `write`

- `write`
  - Saves the active buffer when it already has a path.
  - Errors for unnamed buffers.

- `write <path>`
  - Saves to a new path.
  - Errors if the destination already exists (no overwrite).

### `edit`

- `edit`
  - Opens a new unnamed buffer in a new tab.

- `edit <path>`
  - Switches to an already open buffer for that path when present.
  - Otherwise opens the path in a new tab.

## Quoted Paths

Commands accept quoted path arguments:

- `edit "notes/today file.txt"`
- `write "output/new name.txt"`
