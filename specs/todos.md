# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

## Features

- support raw text insertion by paste
- session support
- relative numbers
- refactor SyntaxCache into a BufferCache container with both syntaxcache and indent scope cache

# Bugs

- undo should store syntax cache because undo requires a full rehighlight
- inserting de-dented text is still leaving scope from above completed below the line
- still cannot scan immediately to end of long file

