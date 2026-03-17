# Vim Motions in urvim

This document describes the motions implemented in urvim and how they differ from Vim behavior.

## Count Support

urvim supports count prefixes for most motions. There are two types of count behaviors:

1. **Repeatable motions** (h, j, k, l, w, b, e, W, B, E): The motion is executed `count` times from the current position.

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

## Operator Motion Differences Summary

| Motion | urvim Behavior | Vim Behavior |
|--------|---------------|---------------|
| h/l    | Grapheme-aware | Byte-aware |
| w/b/e  | Non-word chars = separate words | Non-word chars treated as delimiters |
| $      | Wraps to next line when at EOL | Stays at EOL |
| 0      | Wraps to previous line at column 0 | Stays at column 0 |
| ^      | Wraps to previous line when at first non-ws | Stays at current position |
