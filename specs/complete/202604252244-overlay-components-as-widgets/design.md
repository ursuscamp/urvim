# Overlay Components as Widgets - Technical Design

## Architecture Overview
This refactor extracts command-line and notification overlay behavior into widget components and keeps `Layout` focused on orchestration.

Target structure:
1. Overlay widgets encapsulate stateful event handling and rendering.
2. Layout/root routes UI events with overlay-first precedence.
3. Layout delegates overlay rendering to widget render hooks.
4. Shared floating frame primitive remains the common visual container.

## Interface Design
### Overlay widget interfaces
Each overlay implements the existing `Widget` trait:
- `handle_ui_event(&UiEvent, &mut UiContext) -> UiEventResult`
- `layout(UiConstraints) -> Size` (as needed)
- `render_widget(&mut Screen, UiRect, &UiContext)`
- `focus_policy() -> FocusPolicy`

### Command-line widget interface (public behavior)
- Open/close lifecycle controlled by layout command routing.
- Accepts key/paste events while active.
- Emits command intents for notification errors as today.

### Notification widget interface (public behavior)
- Passive overlay widget.
- Processes tick events for prune/advance behavior.
- Renders current active notification in top-right floating frame.

## Data Models
### `CommandLineWidgetState`
- input buffer
- history and history cursor
- open/active state
- optional visual cursor position for terminal cursor placement

Constraints:
- Session-only history.
- Always closes on submit attempt.

### `NotificationWidgetState`
- references or adapters around existing notification queue/state APIs
- no user-input focus state

Constraints:
- Existing TTL/queue semantics unchanged.

## Key Components
### 1) Command-line widget module
Responsibilities:
- Own command-line input state and key/paste handling.
- Execute parsed commands through existing layout/editor integration points.
- Render centered floating bordered UI.

Dependencies:
- command parser/executor
- floating window abstraction
- intent/command dispatch types

### 2) Notification banner widget module
Responsibilities:
- Consume/prune notification state on ticks.
- Render top-right floating bordered banner.

Dependencies:
- notification state APIs
- floating window abstraction

### 3) Layout overlay host/orchestrator
Responsibilities:
- Hold overlay widget instances.
- Route overlay-first events.
- Merge emitted intents from widget handling.
- Delegate overlay rendering during frame render.

Dependencies:
- widget modules
- unified intent dispatch

## User Interaction
No behavior changes expected:
- `:` opens command-line overlay in centered floating border.
- command-line interactions remain unchanged.
- notification queue progression and banner placement remain unchanged.

## External Dependencies
- No new external crates required.

## Error Handling
- Widget event handlers return `NotHandled` when inactive/non-applicable.
- Command parse/execute failures continue emitting notification intents.
- Notification widget gracefully handles empty queue and small terminal bounds.

Recovery:
- Overlay widgets must not panic on malformed input or constrained viewport sizes.

## Security
- No change in trust boundaries.
- Command-line remains editor-command-only (no shell execution).

## Configuration
- No new configuration options.
- Existing theme and unicode border settings continue to flow through floating frame rendering.

## Component Interactions
1. Terminal event converted to `UiEvent`.
2. Layout routes event to overlay widgets first.
3. Active widget handles event and may emit intents.
4. Layout dispatches intents.
5. During render, layout draws base content then delegates overlay widget rendering.
6. Layout resolves visual cursor, preferring active command-line widget cursor when present.

## Platform Considerations
- Terminal key variations remain handled through existing key canonicalization.
- Overlay widgets must clamp geometry for small terminal sizes.
- No platform-specific behavior changes are intended.
