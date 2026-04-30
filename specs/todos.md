# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- session support
- should panes hold dynamic widgets?
- optimize syntax highlighting by only rehighlighting lines that need to be rehighlighted
    - probably need to re-highlight until line state matches previous
- get rid of centralized backgroud job system
    - create background threads in appropriate places:
        - in bufferpool for cache refreshes
        - in picker to syntax caching
        - in pickers for streaming results

- build-time compiled themes and syntax grammar

# Bugs

