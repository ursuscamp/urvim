# Line Indent Controls - Implementation Tasks

## Overview
Implement line-wise indent increase/decrease in normal mode plus insert-mode reverse-tab and indentation-aware backspace. The work should reuse the existing indentation detection helpers, preserve line contents outside leading whitespace, and add regression coverage for both movement and editing behavior.

## Backend
- [x] **1.** Add reusable line indentation rewrite helpers to the buffer layer (depends on: none)
  - [x] **1.1** Add helpers that measure the current line's leading-whitespace width and resolve a single indent increment for the active buffer.
  - [x] **1.2** Add helpers that remove one indent increment from the start of a line without touching the rest of the text.
  - [x] **1.3** Add helpers that insert one indent increment at the start of a line using the current indentation style.
  - [x] **1.4** Cover the helpers with unit tests for spaces, tabs, mixed indentation, and underflow at column 0.

- [x] **2.** Add normal-mode line indentation commands (depends on: 1)
  - [x] **2.1** Add action kinds and key bindings for `<<` and `>>`.
  - [x] **2.2** Apply the commands to the current line plus any counted continuation lines.
  - [x] **2.3** Preserve non-indentation text and keep partial dedents safe when a line has less whitespace than one full step.
  - [x] **2.4** Update `docs/motions.md` to document the new normal-mode indentation commands and their count behavior.

- [x] **3.** Add insert-mode reverse-tab behavior (depends on: 1, 2)
  - [x] **3.1** Add support for canonical `<S-Tab>` input, normalizing any terminal-specific reverse-tab event into that binding if needed.
  - [x] **3.2** Bind insert-mode `<S-Tab>` to a line dedent action that keeps insert mode active.
  - [x] **3.3** Make insert-mode backspace dedent the current line while the cursor is still in leading indentation.
  - [x] **3.4** Fall back to existing character deletion once the cursor is past the indentation region.

## Testing
- [x] **4.** Add regression tests for normal-mode indentation commands (depends on: 2)
  - [x] **4.1** Verify `<<` decreases indentation on the current line.
  - [x] **4.2** Verify `>>` increases indentation on the current line.
  - [x] **4.3** Verify counts apply the shift across multiple consecutive lines.
  - [x] **4.4** Verify dedent stops cleanly at the left edge without altering line content.

- [x] **5.** Add regression tests for insert-mode reverse-tab and backspace behavior (depends on: 3)
  - [x] **5.1** Verify `<S-Tab>` dedents a line without leaving insert mode.
  - [x] **5.2** Verify `<S-Tab>` works when the cursor is not at column 0.
  - [x] **5.3** Verify backspace inside leading indentation removes indentation in steps.
  - [x] **5.4** Verify backspace outside leading indentation keeps ordinary deletion behavior.

- [x] **6.** Run project checks and fix regressions (depends on: 1, 2, 3, 4, 5)
  - [x] **6.1** Run `cargo check` and fix any build or warning regressions.
  - [x] **6.2** Run the relevant buffer, editor, and window tests for the new indentation behavior.

## Completion Summary

| Area | Tasks | Done | Status |
| --- | --- | ---: | --- |
| Buffer helpers | 1 | 1 | Done |
| Normal-mode indent commands | 1 | 1 | Done |
| Insert-mode reverse-tab | 1 | 1 | Done |
| Testing | 3 | 3 | Done |
| Total | 6 | 6 | Done |
