# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- operations + text objects
- registers
- jump list
- support raw text insertion by paste
- improved layout
- session support

backgroud jobs:

- syntax highlight all files at startup (background priority for all initially invisible files)
- for visible files, highlight first visible part synchronously if it starts at first line
- should we clone lines as strings arc<str> like we are?

# Bugs

- "cw" at the end of the line joins the next line, but it shouldn't
