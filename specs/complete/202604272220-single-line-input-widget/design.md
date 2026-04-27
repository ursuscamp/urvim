# Single-Line Input - Technical Design

## Architecture Overview
The single-line input widget will be a reusable, shell-style text entry component shared by command-line and picker overlays. It will own the editable string, cursor position, and key handling policy while exposing a small consumer-facing surface for live updates and custom key interception.

The widget should follow a simple processing order:
1. Give the consumer first chance to override the key.
2. Apply built-in shell-style line-editing behavior if the key is not overridden.
3. Treat any remaining non-special key as text input.
4. Leave `Enter` and `Esc` as no-ops unless the consumer overrides them.

This keeps command-line and picker input behavior consistent while still allowing each caller to layer on submission, cancellation, history, or selection semantics.

## Interface Design

### Core widget state
The widget should own only the state needed for one-line editing:
- current text buffer
- cursor byte/grapheme position
- optional consumer callbacks for change notification and key interception
- optional display prompt prefix shown before the editable text

Suggested shape:
```rust
pub struct SingleLineInputWidget {
    text: String,
    cursor: usize,
}
```

### Consumer hooks
The consumer should be able to intercept any key and observe text changes.

Suggested interaction surface:
```rust
impl SingleLineInputWidget {
    pub fn new(initial_text: impl Into<String>) -> Self;
    pub fn text(&self) -> &str;
    pub fn cursor(&self) -> usize;
    pub fn set_prompt(&mut self, prompt: impl Into<String>);
    pub fn set_on_change(&mut self, on_change: impl FnMut(&str) + 'static);
    pub fn set_key_override(&mut self, key_override: impl FnMut(KeyEvent) -> Option<bool> + 'static);
    pub fn handle_key(&mut self, key: KeyEvent) -> bool;
}
```

Behavioral contract:
- `set_key_override` receives the normalized key event.
- Returning `Some(true)` marks the key handled.
- Returning `Some(false)` or `None` allows built-in handling to continue.
- `set_on_change` is called after any text mutation.
- `set_prompt` updates the display-only prefix rendered before the editable text.

The exact callback types can follow the project’s existing key event and widget conventions, but the semantics above must remain intact.

### Built-in key handling
The widget should provide shell-style defaults for common single-line editing:
- `Backspace`: delete the character before the cursor.
- `Delete`: delete the character under the cursor.
- `Ctrl-W`: delete the word immediately before the cursor.
- `Ctrl-U`: delete from the cursor to the start of the line.
- `Ctrl-A` or `Home`: move to the start of the line.
- `Ctrl-E` or `End`: move to the end of the line.
- `Ctrl-B` or `Left`: move left by one character.
- `Ctrl-F` or `Right`: move right by one character.
- `Alt-B` or `Ctrl-Left`: move left by one word.
- `Alt-F` or `Ctrl-Right`: move right by one word.

`Enter` and `Esc` must not submit or cancel by default. They remain available for consumer override only.

### Default text insertion
If a key is not overridden and is not one of the built-in editing keys, the widget should insert the key’s text representation at the cursor.

This is what makes the component reusable for command-line and picker entry without requiring every consumer to reimplement basic typing behavior.

### Prompt rendering
The widget should render a consumer-supplied prompt prefix before the editable text.

Prompt rules:
- the prompt is display-only
- the prompt does not count as part of the editable text buffer
- the cursor never enters the prompt region
- command-line overlays can supply `:` while picker overlays can supply `>`

## Data Models

### Editing state
The widget only needs enough state to edit a single line safely:
- `text`: the editable string
- `cursor`: current insertion point

The widget should preserve valid cursor placement after every mutation.

### Key override outcome
The override callback should support three outcomes:
- handled by the consumer
- fall through to built-in widget handling
- fall through to normal text insertion

No extra command routing or submission state is required inside the widget itself.

## Key Components

### Single-line input widget
Responsibilities:
- maintain the editable text and cursor
- apply shell-style editing defaults
- call consumer hooks when text changes or keys are overridden
- stay agnostic about whether the caller is a command line, picker, or future overlay

### Command-line integration
The command line should embed the widget as its editing core, set the prompt to `:`, and supply consumer behavior for submission, cancellation, and any future command-line-specific controls.

The widget itself should not encode command semantics.

### Picker integration
The picker should embed the same widget, set the prompt to `>`, and use the change callback to refresh filtering or search state as the query changes.

The widget itself should not know about result lists, highlighting, or selection state.

## User Interaction

### Shell-style editing feel
The widget should behave like a conventional shell prompt:
- typing appends or inserts text at the cursor
- word movement and deletion are available without switching modes
- start/end navigation is immediate
- `Enter` and `Esc` do nothing unless the consumer overrides them

### Override behavior
When a consumer overrides a key:
1. the override receives the raw key event,
2. the widget skips built-in handling if the override claims the key,
3. otherwise the widget falls back to its defaults.

This lets command-line and picker overlays customize behavior without forking the input core.

## External Dependencies
- Existing `Widget` trait and overlay composition paths.
- Existing key event representation and canonicalization.
- Existing rendering primitives for one-line text entry.

## Error Handling
The widget should fail safely in the following cases:
- If a key maps to no textual input and is not overridden, it should be ignored rather than panicking.
- If a deletion or movement would cross a boundary, it should clamp to the nearest valid cursor position.
- If the widget receives text changes while the cursor is already at an edge, it should preserve a valid cursor position.
- If a consumer callback misbehaves, the widget should not corrupt its own text state.

## Security
The widget does not execute commands or interpret text beyond editing. It should:
- treat all input as plain user text unless a consumer handles it otherwise,
- avoid introducing hidden command execution paths,
- preserve the same terminal safety constraints as the rest of the editor.

## Configuration
No new configuration is required. The widget should inherit the existing editor theme and layout styling from its host overlay.

## Component Interactions
1. Command-line or picker overlay creates the widget with initial text.
2. Overlay routes key events to the widget first.
3. The widget applies consumer overrides, then built-in editing, then plain insertion.
4. The widget emits change callbacks when text is mutated.
5. The overlay uses the current text for submission, filtering, or display.

The widget remains reusable because it never owns overlay-specific behavior.

## Platform Considerations
The widget should work in the same terminal environments as the rest of urvim:
- ASCII-only terminals
- Unicode-capable terminals
- narrow terminal widths where one-line content may need to be clipped or scrolled by the host overlay

Cursor movement and deletion should remain grapheme-safe where the project’s existing key handling already expects grapheme-aware behavior.
