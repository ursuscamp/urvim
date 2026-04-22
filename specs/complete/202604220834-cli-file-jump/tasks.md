# Tasks: CLI File Jump Positioning

## Checklist

- [x] Add a CLI argument parser that recognizes trailing numeric `:line[:column]` suffixes.
- [x] Keep colon-containing paths intact when the trailing suffix is not numeric.
- [x] Convert 1-based CLI coordinates into internal buffer coordinates and clamp them.
- [x] Apply the requested cursor position after opening the file.
- [x] Run cursor sync after positioning and before the buffer becomes active.
- [x] Add parser unit tests for path-only, line-only, line-column, and colon-path cases.
- [x] Add cursor placement tests for out-of-range line and column inputs.
- [x] Verify the startup path still opens files normally when no position suffix is present.

## Completion Notes

- Implemented CLI file-position parsing, initial cursor placement, cursor sync, and tests.
