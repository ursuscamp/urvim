# Confirmation Box - Technical Design

## Architecture Overview
The confirmation box will be a reusable modal UI component that lives in the editor's overlay layer and participates in the existing `Widget` and `Intent` model. It will present a short yes/no query, capture keyboard input while active, and either emit a caller-supplied positive intent or cancel without emitting an intent.

The component should be reusable by any flow that needs a simple proceed-or-cancel decision. The immediate first consumer is the quit-with-unsaved-files path, but the component itself should not know about quitting, buffers, or save semantics.

The design should follow the same overall shape as the command-line overlay and notification widget:
- a small reusable widget/state pair for prompt behavior,
- overlay-first routing while the prompt is active,
- rendering in a bordered floating window,
- intent emission back to the root dispatcher when the user confirms.

## Interface Design

### Confirmation prompt construction
The confirmation box should be constructed from a query message and a positive intent provided by the caller.

Suggested constructor shape:
```rust
pub struct ConfirmationBox {
    pub fn new(query: impl Into<String>, positive_intent: impl Into<Intent>) -> Self;
}
```

### Public behavior
The component should expose behavior that lets the layout layer:
- determine whether the prompt is currently active,
- route key and paste events while active,
- render the prompt inside the existing floating-window system,
- consume the prompt once the user makes a choice.

Suggested interaction surface:
```rust
impl ConfirmationBox {
    pub fn is_open(&self) -> bool;
    pub fn query(&self) -> &str;
    pub fn handle_ui_event(&mut self, event: &UiEvent, ctx: &mut UiContext) -> UiEventResult;
    pub fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext);
}
```

### Event contract
While open, the prompt should treat the following inputs as follows:
- `Y` / `y`-style yes input: confirm and return the stored positive intent.
- `N` / `n`-style no input: cancel and return no intent.
- `Enter`: confirm and return the stored positive intent.
- `Esc`: cancel and return no intent.
- Other keys: ignore or remain open without producing an intent.

The component should not emit multiple intents for a single confirmation action.

## Data Models

### ConfirmationBox state
The prompt needs only a small amount of state:

```rust
pub struct ConfirmationBox {
    query: String,
    positive_intent: Intent,
    open: bool,
}
```

Field responsibilities:
- `query`: the message shown to the user.
- `positive_intent`: the intent returned when the user confirms.
- `open`: whether the prompt is still active and should receive events/rendering.

### Confirmation outcome
The event handler should communicate results using the existing `UiEventResult` contract:
- `Handled(vec![positive_intent])` for confirmation.
- `Handled(Vec::new())` for cancellation.
- `NotHandled` only when the event is unrelated or the prompt is not active.

No schema or persistent storage changes are required.

## Key Components

### Confirmation box widget
The confirmation box widget is responsible for:
- holding the query text and positive intent,
- deciding whether a key confirms or cancels,
- closing itself after the user makes a decision,
- rendering a short prompt with clear yes/no affordances.

It depends on:
- `Intent` for the confirm action payload,
- `UiEvent` / `UiEventResult` for input handling,
- `Screen` and `UiRect` for rendering,
- the floating window helper for frame geometry and borders.

### Layout overlay integration
The layout layer should host the confirmation box as an overlay, similar to the command-line surface. When the prompt is open:
- it receives key events before the base editor,
- it can prevent the quit flow from continuing until the user responds,
- confirmation should produce the stored intent so the root dispatcher can act on it.

### Quit flow integration
The quit flow should begin with a `TryQuit`-style intent or command that represents an attempt to exit. The dispatcher should inspect the editor state when that request is received:
- if there are no modified buffers, it should emit the real quit intent immediately,
- if modified buffers exist, it should open a confirmation box and supply `Intent::Command(Command::Quit)` as the positive intent.

The confirmation component itself should remain unaware of modified-buffer detection and should only return the intent it was constructed with.

## User Interaction

### Prompt content
The prompt should clearly present:
- the query message,
- a visible yes/no cue such as `[Y]es / [N]o`,
- keyboard support for `Enter` and `Esc`.

The interaction should be simple and consistent with the rest of the editor's keyboard-driven UI.

### Confirm path
When the user chooses Yes:
1. the prompt closes,
2. the stored positive intent is returned,
3. the caller can forward that intent into the normal dispatch path.

### Cancel path
When the user chooses No:
1. the prompt closes,
2. no intent is returned,
3. the caller remains in its current state.

### Focus and routing
The confirmation box should be passive from the perspective of normal window focus, but active while visible for overlay event routing. This keeps the prompt modal without introducing a full focusable pane.

## External Dependencies
- Existing `Intent` and `Command` types in `src/ui/mod.rs`.
- Existing `Widget` trait and `UiEvent` routing.
- Existing floating window frame helpers in `src/ui/floating_window.rs`.
- Existing screen rendering primitives and theme/style resolution.
- Existing modified-buffer tracking used by the quit flow.

## Error Handling
The confirmation box should fail safely in the following situations:
- If the terminal is too small to render a bordered prompt, the prompt should not panic; it may decline to render or fall back to the minimum visible frame that fits.
- If the prompt receives unrelated input while open, it should remain open and continue waiting for a valid decision.
- If the caller supplies an intent that the dispatcher later cannot execute, that failure should be handled by the normal intent dispatch path, not by the confirmation box.
- If the prompt is closed or dismissed, it should not emit stale intents afterward.

## Security
The confirmation box does not introduce new security-sensitive behavior. Still, it should:
- avoid interpreting the query text as executable content,
- avoid mutating editor state except when the user confirms,
- rely on the existing intent dispatch system rather than introducing a separate side channel.

## Configuration
No new user-facing configuration is required for the initial version.

If future behavior needs to vary, the prompt should continue to honor existing editor-wide styling and border configuration, such as the current border glyph mode and theme resolution.

## Component Interactions
1. A caller detects that an action needs confirmation.
2. The caller constructs a confirmation box with a query message and a positive intent.
3. The layout layer activates the prompt as the top-most overlay.
4. Key events are routed to the prompt first.
5. The prompt either returns the positive intent or returns no intent and closes.
6. The root dispatcher executes the returned intent using the existing intent pipeline.

This interaction should preserve the separation between prompt UI and the action that the positive intent represents.

## Platform Considerations
The confirmation box should work in the same terminal environments as the rest of urvim:
- text-mode terminals with ASCII borders,
- terminals with Unicode border support,
- small terminals where the prompt may need to shrink or refuse to render,
- keyboard-only interaction without requiring mouse support.

The component should follow the same rendering constraints and grapheme-safe text handling conventions used elsewhere in the editor.
