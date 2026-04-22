# Design: CLI File Jump Positioning

## Overview

Extend CLI file argument parsing so a trailing `:line[:column]` suffix is recognized when the suffix is numeric. After opening the file, apply the requested position, clamp it to the buffer, and run cursor sync before the window renders the buffer.

## Parsing Rules

- Split the argument from the end, not the beginning.
- If the last colon-separated segment is numeric, treat it as `line`.
- If the segment before that also exists and is numeric, treat it as `column`.
- Any non-numeric trailing segment means the full argument remains a path.
- This preserves paths that contain colons as long as the final suffix is not a valid numeric position.

## Position Mapping

- Convert CLI `line` and `column` from 1-based values to internal buffer coordinates.
- Clamp line to the available buffer line range.
- Clamp column to the line length before cursor sync.
- If the file is empty or the requested line is beyond the last line, place the cursor on the nearest valid line first, then clamp the column.

## Cursor Sync

- After the cursor is assigned its target position, run cursor sync on that position.
- Cursor sync is the final normalization step before the cursor is stored or the window becomes active.
- This ensures the restored cursor lands on a valid grapheme boundary and cannot point at an invalid byte offset after path-based jumps.

## Implementation Surface

- Add CLI parsing support in the startup/file-open path.
- Reuse the existing open-file flow after splitting path and position data.
- Add a small position parser helper if the current CLI parsing code becomes clearer with one.

## Testing Strategy

- Unit test the parser with path-only, line-only, line-column, and colon-containing path cases.
- Unit test clamping behavior for short files and overlarge coordinates.
- Add an integration-level check for the startup path that confirms the selected cursor position is synced after the jump.
