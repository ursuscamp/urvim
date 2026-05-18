# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- should panes hold dynamic widgets?
- build-time compiled themes and syntax grammar
- buffer selector
- folding
- show code action modification in preview window
- code action hints for the current line (with ghost text)
- auto-completion

# Bugs

# Refactors

- lots of repetition in globals.rs, maybe modularize it?
- look for places where we clip text at width, replace them with a utility method (if it exists) or a trait
- refactor other pickers with formatted line templates
- refactors to prevent thread contention/locks for tests
