# Vim Motions in urvim

This document describes the motions implemented in urvim and how they differ from Vim behavior.

## Supported Motions Cheat Sheet

| Motion | Description |
|--------|-------------|
| `h` | Move left |
| `j` | Move down |
| `k` | Move up |
| `l` | Move right |
| `w` | Word forward |
| `b` | Word backward |
| `e` | Word end |
| `W` | BigWord forward |
| `B` | BigWord backward |
| `E` | BigWord end |
| `0` | Line start (column 0) |
| `^` | Line content start (first non-whitespace) |
| `$` | Line end (last non-whitespace) |
| `gg` | Go to first line (or line N with count) |
| `G` | Go to last line (or line N with count) |
| `H` | Move to top of viewport |
| `M` | Move to middle of viewport |
| `L` | Move to bottom of viewport |
| `{` | Move to blank line before the previous paragraph |
| `}` | Move to blank line before the next paragraph |
| `a` | Append after cursor (enter insert mode) |
| `A` | Append to line end (enter insert mode) |
| `I` | Insert at line start (enter insert mode) |
| `J` | Join lines with space |
| `gJ` | Join lines without space |
| `dd` | Delete line (or N lines with count) |
| `cc` | Change line: delete line(s) and enter insert mode, leaving one blank line |
| `C` | Change to end of line: delete from cursor to EOL and enter insert mode |
| `o` | Open line below: create new empty line below and enter insert mode |
| `O` | Open line above: create new empty line above and enter insert mode |
| `%` | Jump to matching bracket (parentheses, square brackets, curly braces) |
| `f` | Find forward: move to next occurrence of character |
| `F` | Find backward: move to previous occurrence of character |
| `t` | Till forward: move to position before next occurrence of character |
| `T` | Till backward: move to position after previous occurrence of character |

## Count Support

urvim supports count prefixes for most motions. There are two types of count behaviors:

1. **Repeatable motions** (h, j, k, l, w, b, e, W, B, E, dd, cc, o, O): The motion is executed `count` times from the current position.

2. **Line actions** (0, $, ^): The count specifies an absolute 1-indexed line number to jump to, then performs the action on that line.

> Note: urvim limits counts to values 1-9999 to prevent excessive operations.

## Basic Cursor Movements

### h - Move Left

Moves the cursor left by one grapheme (Unicode-aware character).

- **Count**: Yes - repeats `count` times
- **Vim difference**: urvim is grapheme-aware (handles multi-byte characters like emoji properly), while Vim operates on bytes.

### j - Move Down

Moves the cursor down one line, preserving the visual column position.

- **Count**: Yes - moves down `count` lines
- **Vim difference**: urvim uses a "remembered column" mechanism to preserve the visual column when moving vertically, similar to Vim.

### k - Move Up

Moves the cursor up one line, preserving the visual column position.

- **Count**: Yes - moves up `count` lines
- **Vim difference**: Same behavior as j for column preservation.

### l - Move Right

Moves the cursor right by one grapheme (Unicode-aware character).

- **Count**: Yes - repeats `count` times
- **Vim difference**: urvim is grapheme-aware (handles multi-byte characters like emoji properly), while Vim operates on bytes.

## Word Motions

Word motions use the concept of "words" defined as sequences of alphanumeric characters plus underscore.

### w - Word Forward

Moves to the start of the next word.

- **Count**: Yes - moves to the start of the `count`th next word
- **Vim difference**: In urvim, non-word characters (like `---`) are treated as separate words. In Vim, `w` on `hello---world` from 'h' would go to 'w'; in urvim, it goes to the first `-`.

Example: `hello---world` at 'h':
- Vim: `w` -> 'w' (start of "world")
- urvim: `w` -> first '-' (each '-' is a separate word)

### b - Word Backward

Moves to the start of the previous word.

- **Count**: Yes - moves back `count` words
- **Vim difference**: Same non-word handling as `w`.

### e - Word End

Moves to the end of the current word or next word.

- **Count**: Yes - moves to the end of the `count`th word
- **Vim difference**: Same non-word handling as `w`.

Example: `hello---world` at first '-' (position 5):
- Vim: `e` -> last '-' (position 7)
- urvim: `e` -> last '-' (position 7) - same behavior

## BigWord Motions

BigWord motions treat any non-whitespace sequence as a single word.

### W - BigWord Forward

Moves to the start of the next BigWord (non-whitespace sequence).

- **Count**: Yes
- **Vim difference**: Essentially the same as Vim.

### B - BigWord Backward

Moves to the start of the previous BigWord.

- **Count**: Yes
- **Vim difference**: Essentially the same as Vim.

### E - BigWord End

Moves to the end of the current/next BigWord.

- **Count**: Yes
- **Vim difference**: Essentially the same as Vim.

## Line Motions

### $ - Line End

Moves the cursor to the last non-whitespace character of the current line.

- **Count**: As a line action - goes to line `count` (1-indexed), then moves to end of that line
- **Vim difference**: In urvim, if already at the end of the current line, it wraps to the end of the next line. In Vim, `$` stays at the end of the current line.

Example: On "hello" (cursor at 'e'):
- Vim: `$` -> after 'o' (stays on same line)
- urvim: `$` -> after 'o' (same)

Example: On "hello" (cursor at or past 'o'):
- Vim: `$` -> stays at end of line
- urvim: `$` -> wraps to end of next line (if available)

### 0 - Line Start

Moves the cursor to absolute column 0 (the start of the line).

- **Count**: As a line action - goes to line `count` (1-indexed), column 0
- **Vim difference**: In urvim, if already at column 0, it wraps to the previous line's column 0. In Vim, pressing `0` at column 0 does nothing.

Example: At column 0:
- Vim: `0` -> stays at column 0
- urvim: `0` -> wraps to previous line (if available)

### ^ - Line Content Start

Moves the cursor to the first non-whitespace character of the current line.

- **Count**: No count support (treated as regular motion)
- **Vim difference**: In urvim, if already at the first non-whitespace, it wraps to the previous line's first non-whitespace. In Vim, `^` at the first non-whitespace does nothing.

Example: At first non-whitespace of line 1:
- Vim: `^` -> stays at current position
- urvim: `^` -> wraps to previous line's first non-whitespace

### gg - Go to First Line

Moves the cursor to the first line of the file.

- **Count**: Yes - as a line action, goes to line `count` (1-indexed)
- **Vim difference**: Same behavior as Vim

Examples:
- `gg` -> goes to line 1
- `5gg` -> goes to line 5

### G - Go to Line

Moves the cursor to the last line of the file, or to the specified line with a count prefix.

- **Count**: Yes - as a line action, goes to line `count` (1-indexed)
- **Vim difference**: Same behavior as Vim

Examples:
- `G` -> goes to last line
- `5G` -> goes to line 5

### Column Preservation

Both `gg` and `G` behave like vertical motions (`j`/`k`) for the purposes of column preservation:
- They use the remembered column when moving (like j/k)
- They update the remembered column after moving (so subsequent j/k movements use this column)

This matches Vim's behavior where gg/G set the jump cursor and affect the remembered column.

## Mode-Change Motions

### a - Append After Cursor

Moves the cursor one character to the right and enters insert mode.

- **Count**: No count support
- **Vim difference**: urvim is grapheme-aware (handles multi-byte characters like emoji properly)

Example: On "hel|lo" (cursor before 'l'):
- `a` -> "hell|o" (enters insert mode after the second 'l')

### A - Append to Line End

Moves the cursor to the end of the current line and enters insert mode.

- **Count**: As a line action - goes to line `count` (1-indexed), then moves to end and enters insert mode
- **Vim difference**: urvim is grapheme-aware

Examples:
- `A` -> moves to end of current line, enters insert mode
- `3A` -> goes to line 3, moves to its end, enters insert mode

### I - Insert at Line Start

Moves the cursor to the first non-whitespace character of the current line and enters insert mode.

- **Count**: As a line action - goes to line `count` (1-indexed), then moves to first non-whitespace and enters insert mode
- **Vim difference**: urvim is grapheme-aware

Examples:
- `I` -> moves to first non-whitespace of current line, enters insert mode
- `3I` -> goes to line 3, moves to its first non-whitespace, enters insert mode

## Screen-Relative Motions

These motions move the cursor to positions relative to the currently visible viewport, without scrolling.

### H - Move to Top

Moves the cursor to the first visible line of the viewport (top of screen).

- **Count**: Yes - moves to `count` lines from the top of the viewport
- **Vim difference**: urvim uses capital H (lowercase h is move left)

Examples:
- `H` -> moves to the first visible line
- `3H` -> moves to the 3rd line from the top of the viewport

### M - Move to Middle

Moves the cursor to the middle visible line of the viewport.

- **Count**: No - count is ignored
- **Vim difference**: urvim uses capital M

### L - Move to Bottom

Moves the cursor to the last visible line of the viewport (bottom of screen).

- **Count**: Yes - moves to `count` lines from the bottom of the viewport
- **Vim difference**: urvim uses capital L (lowercase l is move right)

Examples:
- `L` -> moves to the last visible line
- `3L` -> moves to the 3rd line from the bottom of the viewport

### Column Preservation

H, M, and L behave like vertical motions (`j`/`k`) for column preservation:
- They use the remembered column when moving (like j/k)
- They update the remembered column after moving

## Join Line Motions

These motions join multiple lines together.

### J - Join with Space

Joins the current line with the next line(s), inserting a single space between joined lines.

- **Count**: Yes - joins `count + 1` lines (e.g., `3J` joins 4 lines)
- **Cursor position**: After the join, the cursor is positioned at the end of the joined content
- **Vim difference**: Same behavior as Vim

Examples:
- `J` on "hello\nworld" -> "hello world" (cursor at end)
- `2J` on "a\nb\nc\nd" -> "a b c" (joins 4 lines: a, b, c, d with spaces)

### gJ - Join without Space

Joins the current line with the next line(s) without inserting any space between them.

- **Count**: Yes - joins `count + 1` lines
- **Cursor position**: After the join, the cursor is positioned at the end of the joined content
- **Vim difference**: Same behavior as Vim

Examples:
- `gJ` on "hello\nworld" -> "helloworld" (cursor at end)
- `2gJ` on "a\nb\nc\nd" -> "abcd" (joins 4 lines without spaces)

### Edge Cases

- Joining from the last line: No operation (nothing to join with)
- Joining when there are fewer lines than count: Joins all available lines

## Delete Line Operations

These operations delete entire lines from the buffer.

### dd - Delete Line

Deletes the current line (or N lines starting from the cursor position).

- **Count**: Yes - deletes `count` lines starting from the current line
- **Cursor position**: After deletion, the cursor moves to the start of the next line (or the previous line if the deleted line was the last)
- **Vim difference**: Same behavior as Vim

Examples:
- `dd` on line 2 in "a\nb\nc" -> "a\nc" (cursor at line 2, now "c")
- `2dd` on line 1 in "a\nb\nc\nd" -> "c\nd" (deletes lines 1 and 2)
- `dd` on last line "a\nb" -> "a" (cursor at line 1, now "a")

### Edge Cases

- Deleting from the last line: Cursor moves to the previous line
- Deleting when there is only one line: Buffer becomes empty (one empty line remains)
- Count exceeds available lines: Deletes all available lines from the starting position

## Change Line Operations

These operations replace entire lines with a blank line and enter insert mode.

### cc - Change Line

Changes the current line (or N lines starting from the cursor position) by deleting the line(s) and entering insert mode with a single blank line.

- **Count**: Yes - changes `count` lines starting from the current line
- **Mode**: After execution, enters insert mode at the start of the blank line
- **Vim difference**: Same behavior as Vim

Examples:
- `cc` on line 2 in "a\nb\nc" -> "a\n\nc" (cursor in insert mode on empty line 2)
- `2cc` on line 1 in "a\nb\nc\nd" -> "\nd" (lines 1 and 2 replaced with 1 blank line)
- `cc` on last line "a\nb" -> "a\n" (cursor in insert mode on empty line 2)

### Edge Cases

- Changing from the last line: Cursor on blank line at previous position
- Changing when there is only one line: Buffer has one empty line, cursor in insert mode
- Count exceeds available lines: Replaces all available lines with one blank line

### C - Change to End of Line

Changes text from the cursor position to the end of the current line (or N lines) by deleting the text and entering insert mode at the truncation point.

- **Count**: Yes - changes from cursor to end of `count` lines (e.g., `2C` deletes cursor to end of current line plus the next line)
- **Mode**: After execution, enters insert mode at the end of the remaining text
- **Vim difference**: Same behavior as Vim's `c$`

Examples:
- `C` on "hell|o world" -> "hell" (cursor in insert mode after "hell")
- `C` on "|hello" -> "" (cursor in insert mode at beginning of empty line)
- `2C` on "hello| world\nsecond line\nthird" -> "hello" (deletes cursor to end of line 0 plus all of line 1)

### Edge Cases

- Cursor at end of line: No deletion occurs, but enters insert mode at same position (like `a`)
- Count exceeds available lines: Deletes to end of last available line
- Empty buffer: Cursor at position 0, enters insert mode

## Open Line Operations

These operations create new empty lines and enter insert mode.

### o - Open Line Below

Creates a new empty line below the current line and enters insert mode at the start of that line.

- **Count**: Yes - creates `count` empty lines below the current line
- **Mode**: After execution, enters insert mode at column 0 of the new line
- **Vim difference**: Same as Vim.

Examples:
- `o` on line 2 in "a\nb\nc" -> "a\nb\n\nc" (cursor in insert mode on empty line 3)
- `3o` on line 1 in "a\nb\nc" -> "a\n\n\n\nb\nc" (creates 3 empty lines below)

### O - Open Line Above

Creates a new empty line above the current line and enters insert mode at the start of that line.

- **Count**: Yes - creates `count` empty lines above the current line
- **Mode**: After execution, enters insert mode at column 0 of the new line
- **Vim difference**: Same as Vim.

Examples:
- `O` on line 2 in "a\nb\nc" -> "a\n\nb\nc" (cursor in insert mode on empty line 2)
- `3O` on line 2 in "a\nb\nc" -> "a\n\n\nb\nc" (creates 3 empty lines above)

### Edge Cases

- `o` on the last line: Creates new line at the end of the buffer
- `O` on the first line: Creates new line at the beginning of the buffer
- Empty buffer: Both create a single empty line and enter insert mode

## Operator Motion Differences Summary

| Motion | urvim Behavior | Vim Behavior |
|--------|---------------|---------------|
| h/l    | Grapheme-aware | Byte-aware |
| w/b/e  | Non-word chars = separate words | Non-word chars treated as delimiters |
| $      | Wraps to next line when at EOL | Stays at EOL |
| 0      | Wraps to previous line at column 0 | Stays at column 0 |
| ^      | Wraps to previous line when at first non-ws | Stays at current position |
| gg     | Goes to line 1 (or line N with count) | Same |
| G      | Goes to last line (or line N with count) | Same |
| o      | Same as Vim | Same as Vim |
| O      | Same as Vim | Same as Vim |

## Bracket Matching

### % - Jump to Matching Bracket

Moves the cursor to the matching opening or closing bracket.

- **Count**: No - not countable
- **Supported brackets**: `()`, `[]`, `{}`
- **Vim difference**: urvim matches Vim behavior for basic bracket matching

Examples:
- On `(` in `function(foo)` -> jumps to the matching `)`
- On `)` in `function(foo)` -> jumps back to `(`
- On `[` in `[1, 2, 3]` -> jumps to `]`
- On `{` in `{ a: 1 }` -> jumps to `}`

### Edge Cases

- On a non-bracket character: No movement (silent fail)
- No matching bracket exists: No movement (silent fail)
- Nested brackets: Correctly handles nesting (e.g., `((foo))` - first `%` goes to middle, second to end)

## Character Scan Motions

Character scan motions allow quick navigation to or past a specified character in the current line.

### f - Find Forward

Moves the cursor to the next occurrence of the specified character.

- **Count**: Yes - finds the `count`th occurrence
- **Search direction**: Forward from cursor (searches the character after the cursor position)
- **Cursor position**: Lands ON the found character

Examples:
- `f o` on "hello| world" -> "hello w|orld" (cursor on 'o' in "world")
- `2f x` on "xxx" -> lands on third 'x'
- `f z` on "hello" (no 'z') -> cursor stays in place (no movement)

### F - Find Backward

Moves the cursor to the previous occurrence of the specified character.

- **Count**: Yes - finds the `count`th previous occurrence
- **Search direction**: Backward from cursor (searches the character before the cursor position)
- **Cursor position**: Lands ON the found character

Examples:
- `F h` on "|hello" -> cursor on 'h' (stays in place since already at 'h')
- `F e` on "he|llo" -> cursor on 'e' (searches backward, finds first 'e')
- `F z` on "hello" (no 'z') -> cursor stays in place (no movement)

### t - Till Forward

Moves the cursor to the position just before the next occurrence of the specified character.

- **Count**: Yes - till the `count`th occurrence
- **Search direction**: Forward from cursor
- **Cursor position**: Lands one position BEFORE the found character

Examples:
- `t o` on "hel|lo world" -> "hel l|o world" (cursor on 'l' before 'o')
- `2t x` on "x x x" -> cursor on first 'x' (position before second 'x')

### T - Till Backward

Moves the cursor to the position just after the previous occurrence of the specified character.

- **Count**: Yes - till the `count`th previous occurrence
- **Search direction**: Backward from cursor
- **Cursor position**: Lands one position AFTER the found character

Examples:
- `T h` on "he|llo" -> "h|ello" (cursor on 'e', which is after 'h')
- `T e` on "|hello" -> cursor stays (no character before to land after)

### Character Scan Motion Details

- **Case sensitive**: `fX` searches for uppercase 'X', not lowercase 'x'
- **Current line only**: Search is limited to the current line (does not wrap to next/previous line)
- **Not found behavior**: If the target character is not found, the cursor stays in place (no movement)
- **Boundary clamping**: For till motions, if the offset would place the cursor outside the line, it is clamped to the line boundary

### Edge Cases

- **Cursor at line start**: `F` and `T` search before the cursor, so starting at column 0 means no characters before to search
- **Cursor at line end**: `f` and `t` search after the cursor, so starting at the last column means no characters after to search
- **Count exceeds occurrences**: Lands on the last available occurrence (or stays in place if none found)
- **Till at line boundary**: `t` on last char of line lands on last char; `T` on first char stays at column 0

## Repeat Character Search Motions

Character scan motions (f, F, t, T) can be repeated using `;` and `,`.

### ; - Repeat Last Search (Same Direction)

Repeats the last character search motion in the same direction.

- **Count**: Yes - repeats `count` times
- **Search direction**: Same as the original search (f/F/t/T)
- **State persistence**: The last search state persists across mode switches

Examples:
- `f x` then `;` → finds next 'x' forward
- `F x` then `;` → finds previous 'x' backward
- `t x` then `;` → till before next 'x' forward
- `3 f x` then `2 ;` → finds 3rd 'x', then finds 5th 'x'

### , - Repeat Last Search (Opposite Direction)

Repeats the last character search motion in the opposite direction.

- **Count**: Yes - repeats `count` times
- **Search direction**: Opposite of the original search (f/F/t/T)
- **State persistence**: The last search state persists across mode switches

Examples:
- `f x` then `,` → finds previous 'x' backward (opposite direction)
- `F x` then `,` → finds next 'x' forward (opposite direction)
- `t x` then `,` → till after previous 'x' backward
- `T x` then `,` → till before next 'x' forward

### State Persistence

The last character search state is stored globally and persists when:
- Switching to insert mode and back to normal mode
- Using multiple windows (future feature)

This allows you to:
1. Press `f x` to find 'x'
2. Press `i` to enter insert mode
3. Type some text
4. Press `<Esc>` to return to normal mode
5. Press `;` to find 'x' again

### Edge Cases

- **No previous search**: If `;` or `,` is pressed with no previous character search, the cursor stays in place (silent fail)
- **; and , do not update state**: These actions only read from the stored state, never write to it. Pressing `;` after `,` does not change the stored direction.

## Paragraph Motions

Paragraph motions allow navigation between blocks of text separated by blank lines.

### Definitions

- **Paragraph**: A consecutive sequence of non-empty lines (lines with at least one non-whitespace character)
- **Blank line**: A line that is empty or contains only whitespace characters (spaces and/or tabs)

### { - Move to Previous Paragraph

Moves the cursor to the blank line before the previous paragraph.

- **Count**: Yes - moves up `count` paragraphs
- **Behavior**:
  - If on a non-blank line (inside a paragraph), moves to the blank line **before** the current paragraph
  - If on a blank line, moves to the blank line **before** the previous paragraph (skips any non-blank lines above)

Example buffer:
```
Para 1 line 1
Para 1 line 2

Para 2 line 1
```

- `{` on "Para 2 line 1" -> moves to the blank line between Para 1 and Para 2
- `2{` on "Para 2 line 1" -> moves up 2 paragraphs (to blank line before Para 1, which doesn't exist - stays in place)

### } - Move to Next Paragraph

Moves the cursor to the blank line after the next paragraph.

- **Count**: Yes - moves down `count` paragraphs
- **Behavior**:
  - If on a non-blank line (inside a paragraph), moves to the blank line **after** the current paragraph
  - If on a blank line, moves to the next blank line (or blank line after next paragraph if non-blank lines follow)

Example buffer:
```
Para 1 line 1
Para 1 line 2

Para 2 line 1
```

- `}` on "Para 1 line 2" -> moves to the blank line between Para 1 and Para 2
- `}` on the blank line itself -> moves to the next blank line (if any) or stays in place

### Column Preservation

Paragraph motions behave like vertical motions (`j`/`k`) for column preservation:
- They use the remembered column when moving
- They update the remembered column after moving

### Edge Cases

- **No previous/next paragraph**: Cursor stays in place (no movement)
- **Multiple consecutive blank lines**: Treated as a single blank line boundary
- **Whitespace-only lines**: Treated as blank lines
- **Empty buffer**: No movement (cursor stays in place)
