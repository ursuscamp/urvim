# Action Envelope Refactor - Technical Design

## Architecture Overview
Refactor the editor action model from a bare enum into an action envelope that carries three pieces of information:
- the action payload itself
- the mode that created the action, if it is mode-specific
- the mode to switch to after the action is applied, if any

The intent is to make mode-sensitive behavior explicit at the action boundary instead of depending on global mode state or insert-only action variants. The main event loop remains the coordinator, but it should now read the action envelope to decide whether an action is eligible, how it should be applied, and whether a mode transition should happen after execution.

This keeps the system organized around a single concept: an action can record where it came from when that matters and whether it causes a mode change.

## Interface Design
Replace the current `Action` enum with a struct that exposes the action envelope and the payload enum separately.

Suggested shape:
```rust
pub struct Action {
    pub kind: Option<ActionKind>,
    pub from_mode: Option<ModeKind>,
    pub to_mode: Option<ModeKind>,
}

pub enum ActionKind {
    MoveLeft,
    MoveDown,
    MoveUp,
    MoveRight,
    InsertChar(char),
    InsertText(String),
    DeleteBackward,
    DeleteForward,
    // existing motion, operator, repeat, save, undo/redo, and count payloads
}
```

Key interface expectations:
- `ActionKind` carries the actual edit or command intent.
- `from_mode` identifies the mode that produced the action when the action is mode-specific.
- `from_mode: None` means the action is mode-agnostic.
- `to_mode` is `None` for ordinary actions and `Some(mode)` for actions that switch modes.
- A `None` `kind` is reserved for pure mode-transition actions.
- Existing helper queries such as repeat classification, snapshot classification, and cursor bookkeeping move onto `Action` and inspect `kind` plus the mode fields as needed.

The main loop should not need to infer mode transitions from separate switch-only variants. Instead, it should use `to_mode` after a handled action to update the live `Mode` object and the layout status label.

## Data Models
### Action
The action envelope is the unit of dispatch, repeat, and undo bookkeeping.

Fields:
- `kind: Option<ActionKind>`: The action payload or `None` for a pure mode transition
- `from_mode: Option<ModeKind>`: The originating mode when source-mode context matters
- `to_mode: Option<ModeKind>`: The destination mode after the action completes

Constraints:
- `from_mode` is optional and is set only when the action should be constrained to a specific source mode.
- `to_mode` is optional and only set when the action intentionally changes modes.
- `kind: None` is only valid when the action’s purpose is to change modes.

### ActionKind
The payload enum remains the catalog of editor intent.

Responsibilities:
- represent cursor motions
- represent inserts and deletes
- represent save, undo, redo, repeat, and operator actions
- represent counted actions by wrapping an inner `ActionKind` or equivalent payload structure

Constraints:
- The payload enum should not need to know about the current live mode.
- Any mode-specific behavior should be derived from the envelope fields, not by expanding the payload enum into insert-only subtypes.

### Repeat Replay
Repeat state should continue to store the action payload that was actually performed, along with any committed insert text and structural count information already used by dot-repeat.

The replay model should remain compatible with the new action envelope by replaying the payload in the originating mode context and then honoring the recorded destination mode when applicable.

## Key Components
### Mode Implementations
Normal mode and insert mode should construct `Action` values directly with `from_mode` set only when the action should be source-mode constrained.

Responsibilities:
- translate keys into `Action` values
- attach `to_mode` for commands that switch modes
- keep insert-mode auto-pairing behavior as ordinary `InsertChar` and `DeleteBackward` actions whose meaning is interpreted from the action envelope

### Main Event Loop
The main loop remains the place that coordinates mode changes, buffer mutation, and repeat handling.

Responsibilities:
- resolve dot-repeat before dispatch
- reject or ignore actions whose `from_mode` is set and does not match the active mode
- dispatch the `ActionKind` payload to the layout/window layer
- apply `to_mode` after a handled action
- preserve snapshot and repeat bookkeeping after successful edits

### Window and Buffer Editing
Window-level editing should keep handling the actual text mutation and cursor movement. It should inspect the action payload together with the action envelope, not hidden global mode state, for behavior. Mode-sensitive behavior is determined by the envelope and dispatch rules.

Responsibilities:
- apply insertions, deletions, and motions
- preserve undo-friendly cursor updates
- keep insert-mode pairing behavior working when the action envelope indicates insert-mode origin

### Status and Layout Mode Tracking
Layout and status-bar display should continue to reflect the current live mode object. The mode label should be updated only after an action with `to_mode` completes.

## User Interaction
From the user’s perspective, nothing should change in the command vocabulary:
- typing in insert mode still inserts text normally
- supported openers still auto-pair
- matching closers still skip over existing closers
- backspace still removes paired delimiters when appropriate
- `i`/`Esc` style mode changes still work

The difference is internal: mode-aware behavior is inferred from action metadata rather than hidden global state or special insert-only variants.

## External Dependencies
No new external dependencies are required. The refactor should stay inside the existing editor, window, and repeat infrastructure.

## Error Handling
Expected failure cases:
- an action may be created with an incompatible `from_mode`
- a replay action may be requested when no repeat state exists
- an action with `kind: None` but no `to_mode` should be treated as invalid
- a mode transition may be requested after a no-op action

Recovery strategy:
- invalid action envelopes should be ignored or rejected early without mutating the buffer
- mode mismatches should fail closed
- repeat replay should continue to behave as a no-op when no repeat state exists
- undo/redo should not attempt to infer modes from buffer contents

## Security
No new security-sensitive surface is expected.

- No authentication or authorization changes
- No secrets handling changes
- No unsafe code should be introduced
- Mode metadata is internal editor state only

## Configuration
No new configuration options are required for this refactor.

Existing auto-pair configuration should continue to control pairing behavior, but the action-envelope change itself should not introduce new user-facing settings.

## Component Interactions
1. A mode receives a key and builds an `Action` with `from_mode` set when the action should be source-mode constrained.
2. The main loop resolves dot-repeat or direct dispatch.
3. The dispatcher checks whether the action is valid for the active mode.
4. The payload is applied to the layout/window layer.
5. If the action succeeds and `to_mode` is set, the live mode object is replaced and the layout mode label is updated.
6. Snapshot and repeat metadata are recorded from the action that actually ran.

This keeps mode switching and edit application in one linear path instead of spreading mode checks across globals and special cases.

## Platform Considerations
The design is terminal- and Rust-runtime agnostic.

- It should work on all platforms already supported by the editor
- It should not depend on platform-specific keyboard behavior
- It should preserve current behavior for terminal escape sequences and key canonicalization
