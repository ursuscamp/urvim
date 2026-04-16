# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- operations + text objects
- registers
- jump list
- support raw text insertion by paste
- session support

- improved layout
    - cursor blinks rapidly for a moment moving between panes sometimes
    - splits copy the active buffer from the source tabs
    - resize
    - remember split pane focus (going back to a split should go back to the last node in the split that the user used)

- syntax highlighting improvements
    - when editing, the current buffer viewport should be re-highlighted synchronously before re-highlighting asynchronously
    - editing a lot causes backlog in the syntax queue (enter a lot of characters rapidly causes the syntax highlighting job to get backed highlighting old lines)

# Bugs

- "cw" at the end of the line joins the next line, but it shouldn't
