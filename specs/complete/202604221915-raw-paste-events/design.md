# Raw Paste Events - Technical Design

## Architecture Overview
The terminal layer already emits bracketed paste payloads as `Event::Paste(String)`. This feature adds an editor-level raw paste handling path that:
- Accepts `Event::Paste` in insert, normal, and visual modes.
- Applies paste text as a dedicated raw insertion operation that bypasses insert helpers.
- Replaces the active selection in visual mode and exits to normal mode.
- Records each raw paste event as one undo unit.

The design keeps paste parsing in `terminal` unchanged and introduces behavior in the event loop and action dispatch layers.

## Interface Design
### Event loop paste handling
- Input: `Event::Paste(payload)`
- Decision:
  - If active mode is `ModeKind::Insert` or `ModeKind::Normal`, dispatch dedicated raw-paste insertion action.
  - If active mode is `ModeKind::Visual` or `ModeKind::VisualLine`, dispatch dedicated raw-paste replacement action.
- Output: redraw request only when the paste is accepted and applied.

### Action surface
Add a dedicated action kind to separate raw paste semantics from existing typed insert semantics.

Proposed action variants:
- `ActionKind::InsertRawPaste(String)`
- `ActionKind::ReplaceSelectionRawPaste(String)`

Proposed action constructors:
- `Action::insert_raw_paste(text: String) -> Action`
- `Action::replace_selection_raw_paste(text: String) -> Action`

Action metadata behavior:
- `is_snapshottable() == true` for `InsertRawPaste` and `ReplaceSelectionRawPaste` so one accepted paste event creates one undo snapshot boundary.
- `updates_snapshot_cursor() == false` for `InsertRawPaste` and `ReplaceSelectionRawPaste` (cursor is mutated by the edit itself).
- `is_dot_repeat_source() == false` for both raw paste actions (no change to dot-repeat semantics in this feature).

### Window action handling
Add handling for `InsertRawPaste` and `ReplaceSelectionRawPaste` in window action processing:
- Insert full payload verbatim at current cursor.
- Update cursor to end-of-pasted-text position.
- Do not invoke auto-close-pairs logic.
- Do not invoke auto-indent logic.
- For visual replacement, remove active visual selection, insert payload at selection start, clear selection, and transition to normal mode.

## Data Models
No persistent schema changes.

In-memory changes:
- Extend `ActionKind` enum with `InsertRawPaste(String)`.
- Extend `ActionKind` enum with `ReplaceSelectionRawPaste(String)`.
- Extend action behavior match tables (`is_snapshottable`, etc.) to include the new variant.

Constraints:
- Payload is treated as opaque text from terminal bracketed paste.
- Newline bytes in payload map to literal newline insertion in buffer content.

## Key Components
### `src/main.rs`
- Replace current placeholder paste-ignore branch with mode-gated dispatch:
  - Insert/Normal mode: apply raw insert paste action.
  - Visual/VisualLine mode: apply raw replace-selection paste action and transition to normal mode.
- Preserve redraw and cursor-style update flow used by normal action handling.

### `src/editor/action.rs`
- Add `InsertRawPaste(String)` to `ActionKind`.
- Add `ReplaceSelectionRawPaste(String)` to `ActionKind`.
- Add `Action::insert_raw_paste`.
- Add `Action::replace_selection_raw_paste`.
- Update action classification helpers to enforce undo/dot-repeat policy for raw paste.

### `src/window/widget.rs`
- Add `InsertRawPaste` branch that performs direct buffer text insertion and cursor advancement without helper transformations.
- Add `ReplaceSelectionRawPaste` branch that replaces the active visual selection with verbatim text and finalizes in normal mode.

## User Interaction
- Insert mode:
  - Pasted text appears exactly as pasted, including newlines.
  - Auto-pairs and auto-indent are not applied during raw paste insertion.
  - One undo step removes one raw paste event.
- Normal mode:
  - Pasted text appears exactly as pasted, including newlines.
  - Auto-pairs and auto-indent are not applied during raw paste insertion.
  - One undo step removes one raw paste event.
- Visual mode:
  - Active selection is replaced by pasted text.
  - Editor exits to normal mode after replacement.
  - One undo step restores pre-paste text and selection result state via existing undo semantics.

## External Dependencies
No new external dependencies.

Relies on existing terminal bracketed-paste support:
- `Event::Paste(String)` emission in terminal input parser.

## Error Handling
- Empty payload: treated as a no-op, no state mutation.
- Oversized payload behavior remains owned by terminal input safeguards (`MAX_PASTE_SIZE`) before event emission.
- Paste events received in supported modes are processed as mode-specific edits; unsupported modes (if any are added later) should treat paste as a no-op rather than an error.

## Security
- No new file, network, or process access paths.
- Raw paste text is inserted as editor content only; it is never executed as a command.
- Existing buffer text encoding path and terminal decoding behavior remain unchanged.

## Configuration
No new config options.

Existing settings intentionally bypassed during raw paste:
- `auto_close_pairs`
- `auto_indent`

## Component Interactions
1. Terminal parser emits `Event::Paste(payload)`.
2. Main loop receives paste event.
3. Main loop checks active mode:
   - Insert/Normal mode: dispatch `Action::insert_raw_paste(payload)` through layout/window action path.
   - Visual/VisualLine mode: dispatch `Action::replace_selection_raw_paste(payload)` through layout/window action path.
4. Window applies verbatim insertion and updates cursor.
5. For visual replacement, window clears visual selection and switches to normal mode.
6. Action snapshot policy records one undo boundary for the accepted paste event.

## Platform Considerations
- Behavior applies to all terminal platforms where bracketed paste is surfaced as `Event::Paste`.
- No platform-specific keymap or escape-sequence behavior changes are introduced by this feature.
