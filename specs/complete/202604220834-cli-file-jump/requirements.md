# Requirements: CLI File Jump Positioning

## Summary
Support opening files from the CLI with `path/to/file:line[:column]` syntax so the editor can jump directly to a requested location.

## Goals

- Accept a trailing line number and optional column number on CLI file arguments.
- Open the target file and move the cursor to the requested position.
- Clamp out-of-range line and column values to valid buffer bounds.
- Run cursor sync after positioning so the cursor is always stored in a valid state.

## Non-Goals

- No changes to non-CLI file-open entry points.
- No support for multi-location ranges or selections.
- No change to the editor's internal cursor coordinate model.

## User Stories

- As a user, I can launch the editor with `file.txt:12` and land on line 12.
- As a user, I can launch the editor with `file.txt:12:4` and land on line 12, column 4.
- As a user, I can pass a path containing colons, and the editor only interprets the final numeric suffix as position data.

## Functional Requirements

- The CLI must accept file arguments in the form `path/to/file:line[:column]`.
- `line` and `column` are 1-based values in the CLI syntax.
- The parser must treat the final one or two colon-separated segments as position data only when they are numeric.
- If the trailing segment is not numeric, the entire argument must be treated as a path.
- If the final segment is numeric but the preceding segment is not numeric, only the final segment is treated as part of the path.
- The editor must open the file, move the cursor to the requested line and column, and clamp to valid bounds when needed.
- After positioning the cursor, the editor must run cursor sync before the buffer is shown as active.
- When the requested position is beyond the end of file or end of line, the final cursor location must be the nearest valid position.

## Acceptance Criteria

- `file.txt:1` opens `file.txt` and positions the cursor at the first line.
- `file.txt:1:1` opens `file.txt` and positions the cursor at the first line, first column.
- `file.txt:9999:9999` opens the file and clamps the cursor to the last valid line and column.
- `path:with:colons/file.txt:8` preserves the path portion and jumps to line 8.
- `path:with:colons/file.txt:8:3` preserves the path portion and jumps to line 8, column 3.
- After any accepted CLI jump, the cursor passes through cursor sync before rendering.
