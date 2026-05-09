# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- should panes hold dynamic widgets?
- build-time compiled themes and syntax grammar
- buffer selector
- reloading changed files on disk
- examine preview pane and hover pane and see if there are any common patterns which can be extracted
- create a fancy text formatter with layout and styling, use it for Gutter and status bar
- folding

## LSP

# Bugs

# Refactors

- lots of repetition in globals.rs, maybe modularize it?
- look for places where we clip text at width, replace them with a utility method (if it exists) or a trait
- for pickers tab (to switch query modes) is handled in layout. can it be in the picker itself like other keys?
- refactor other pickers with formatted line templates
