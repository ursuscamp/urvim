# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- should panes hold dynamic widgets?
- build-time compiled themes
- buffer selector
- show code action modification in preview window
- code action hints for the current line (with ghost text)
- quick jump
- git picker (with special actions)
- healthcheck

# Bugs

# Refactors

- should markerstore be in the buffercache?
- lot of repetition in globals.rs, maybe modularize it?
- colorscheme picker should duplicate the other pickers

# Syntax Refactor

- nim: Hex, binary, octal, block comments
- fsharp: block comments, octal
- justfile: first character of first dependency targets are highlighted as the main target
- more r token types (functions, maybe others?)
- change to using unique integers for context types
- review with coderabbit cli
