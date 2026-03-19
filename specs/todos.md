# Todos

This is a list, in no particular order, of things that need to be addressed which do not have specs created yet.

- Repeatable commands (including repeatable inserts)
  - Repeatable o/O (inserted text is repeated, along with lines)
- operations + text objects
- registers
- { and } motions
- count with ^ not working as expected
- refactor character iteration where necessary by adding next_cursor/prev_cursor helpers

character scan motions

- cursor should advance by grapheme, not by byte
