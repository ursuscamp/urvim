# Bug Report: "A" Inserts One Character Before End of Line

## Bug Description

When pressing `A` in normal mode, the cursor positions at the last non-whitespace character instead of after the last character of the line. This causes text to be inserted one character before where it should be.

## Steps to Reproduce

1. Open a file with content "hello"
2. In normal mode, press `A`
3. Start typing - the text will be inserted before the 'o', not after it

## Expected Behavior

`A` should move cursor to after the last character of the line (position = line length), matching vim behavior.

## Root Cause

In `Window::process_action()`, `Action::AppendToLineEnd` calls `move_cursor_to_line_end()`. This function uses `buffer.cursor_end_of_line()` which positions at the last **non-whitespace** character (line 883 in buffer.rs: `let end_pos = last_non_ws.unwrap_or(0);`).

However, `A` should position at the actual end of the line (after all characters, including trailing whitespace), which is `line_len` (the position equal to the line's character count).

## Solution

Modify the handler for `Action::AppendToLineEnd` in `Window::process_action()` to set the cursor to `line_len` instead of calling `move_cursor_to_line_end()`.
