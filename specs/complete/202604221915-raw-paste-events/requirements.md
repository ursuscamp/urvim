# Raw Paste Events

## Summary
Add support for terminal raw paste events so urvim can accept pasted text as one insert operation in insert mode, while ignoring raw paste events in non-insert modes.

## Problem Statement
urvim currently lacks explicit raw paste handling semantics. Without dedicated behavior, pasted payloads can be processed as regular input, which may trigger insert-mode helpers that should not run for bulk pasted content and can produce unexpected text edits.

## User Stories
- As an insert-mode user, I want pasted text to be inserted exactly as provided so that my clipboard content is preserved.
- As a normal-mode user, I want raw paste events ignored so that accidental terminal paste data does not modify my buffer.
- As a visual-mode user, I want raw paste events ignored so that selection workflows are not disrupted by unsolicited pasted payloads.
- As an editor user, I want a raw paste to be undone in one step so that I can quickly revert a mistaken paste.

## Functional Requirements
- [ ] **REQ-001**: The editor must accept terminal raw paste events as a distinct input path.
- [ ] **REQ-002**: When a raw paste event is received in insert mode, the full payload must be inserted verbatim at the cursor location, including newline characters.
- [ ] **REQ-003**: Raw paste insertion in insert mode must bypass insert-mode helper behaviors, including auto-pairs and auto-indent.
- [ ] **REQ-004**: Raw paste payload bytes must not be interpreted as editor commands.
- [ ] **REQ-005**: A single raw paste event in insert mode must be recorded as one undo unit.
- [ ] **REQ-006**: When a raw paste event is received in normal mode, the event must be ignored and must not mutate editor state.
- [ ] **REQ-007**: When a raw paste event is received in visual mode, the event must be ignored and must not mutate editor state.

## Non-Functional Requirements
- Reliability: Raw paste handling must be deterministic across repeated runs with identical mode and payload inputs.
- Compatibility: Raw paste support must preserve existing non-paste key input behavior in all modes.
- Usability: Pasted content fidelity must match the terminal-provided payload bytes (subject only to the editor's existing internal text encoding constraints).

## Acceptance Criteria
- [ ] **AC-001**: Given insert mode and payload `hello`, when a raw paste event is processed, then `hello` is inserted at the cursor with no extra inserted characters.
- [ ] **AC-002**: Given insert mode and payload containing newlines, when a raw paste event is processed, then line breaks in the buffer match the payload exactly.
- [ ] **AC-003**: Given insert mode and payload containing pairing characters such as `(` or `"`, when a raw paste event is processed, then no auto-pair helper text is added.
- [ ] **AC-004**: Given insert mode and payload that would normally trigger indent helper behavior, when a raw paste event is processed, then no auto-indent adjustment is applied.
- [ ] **AC-005**: Given insert mode after applying one raw paste event, when undo is executed once, then the entire paste insertion is reverted.
- [ ] **AC-006**: Given normal mode and any raw paste payload, when the raw paste event is processed, then buffer text and editor mode remain unchanged.
- [ ] **AC-007**: Given visual mode and any raw paste payload, when the raw paste event is processed, then buffer text, selection, and mode remain unchanged.

## Out of Scope
- Transforming, sanitizing, or filtering raw paste payloads.
- Adding new user configuration flags for raw paste behavior.
- Enabling raw paste handling in modes other than insert mode.
- Modifying completed historical specs to reflect this feature.

## Assumptions
- The terminal integration layer can surface raw paste payloads distinctly from regular keypress events.
- Existing undo infrastructure supports grouping an externally provided text insertion as one undo unit.
- "Ignored" means no text insertion and no mode/state transition side effects.

## Dependencies
- Terminal input event pipeline support for raw paste event delivery.
- Insert-mode text insertion path with undo tracking integration.
- Existing mode-dispatch input handling for normal, insert, and visual modes.
