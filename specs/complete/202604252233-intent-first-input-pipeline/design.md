# Intent-First Input Pipeline Refactor - Technical Design

## Architecture Overview
This refactor moves the input pipeline to an intent-first contract:

1. Key input is processed by mode/keymap logic.
2. Mode handlers emit a unified intent result (action or command).
3. Root loop processes intent(s) directly through the layout dispatcher.
4. UI orchestration remains command-based; editing remains action-based.

The key architectural objective is to eliminate UI bridge actions and make command intents first-class outputs of input resolution.

## Interface Design
### Input resolution result
Replace action-only completion with intent-aware completion.

Current conceptual shape:
- `HandleKeyResult::Complete(Action)`

Target conceptual shape:
- `HandleKeyResult::Complete(Intent)`
- (optional) batch variant if future mode workflows need multiple intents from one sequence.

Ergonomics:
- `HandleKeyResult` should implement `From<Action>` and `From<Command>`.
- `Intent` should implement `From<Action>` and `From<Command>`.
- Constructors for intent-bearing types should accept generic `Into` payloads so call sites can remain concise without explicit wrapping.

Constraints:
- Partial and invalid-sequence states are preserved.
- Count/operator semantics for edit actions remain unchanged.

### Keymap binding output
Evolve keymap value type from action-only to intent-capable representation.

Preferred model:
- keymaps store `Intent` payloads directly.

Alternative compatibility model:
- keymaps store a small enum (`BindingPayload`) that converts to `Intent` at mode boundary.

### Dispatch contracts
- Main loop consumes emitted intents without action-only conversion shims.
- Layout retains `dispatch_intent(&Intent)` as the unified executor.

## Data Models
### `HandleKeyResult`
- Add/replace completion variant for intent payload.
- Preserve `WaitForMore` and `InvalidSequence` variants.

### Binding payload model (if introduced)
Fields/variants:
- Action payload variant
- Command payload variant

Constraints:
- Must be trivially convertible to `Intent`.
- Must preserve existing clone/equality behavior needed by tests.

### `Command` enum
- Add explicit command variants for command-line open, pane/split layout operations, wrap toggling, and quit orchestration (for example `Command::OpenCommandLine`, pane focus/resize/split variants, and `Command::Quit`).
- Keep existing notification command variants.
- Preserve edit-semantic actions such as `InsertChar` as actions rather than commands.

## Key Components
### 1) Editor mode handling
Responsibilities:
- Resolve key sequences and counts.
- Emit intent-based completion values.
- Provide ergonomic constructors and `From` conversions for intent-bearing results.

Dependencies:
- keymaps
- mode-local state (counts, pending operators, pending sequences)

### 2) Keymap subsystem
Responsibilities:
- Match canonical key sequences.
- Return intent-capable payloads.

Dependencies:
- canonical key parser
- trie/character-scan keymap implementations

### 3) Main event loop
Responsibilities:
- Route overlay UI events first.
- Process emitted intents from key handling without action-only assumptions.

Dependencies:
- layout dispatch
- cursor style/mode transition handling
- repeat/snapshot integration

### 4) Layout command dispatcher
Responsibilities:
- Execute UI commands such as opening overlays, managing panes/splits, toggling wrap, enqueueing notifications, and quitting.
- Continue action routing for edit semantics.

Dependencies:
- overlay state
- notification system
- active window group

## User Interaction
User-facing behavior is unchanged:
- `:` opens command-line overlay.
- Editing keys behave as before.
- Overlay and notification routing behave as before.

The change is architectural: how these behaviors are represented and routed internally.

## External Dependencies
- No new external crates required.

## Error Handling
- Invalid key sequences continue to return invalid/wait states without panics.
- Unknown command variants (if any internal mismatch occurs) should fail safely with clear logs and no crash.
- Migration should include compatibility checks in tests to detect dropped intent paths.

## Security
- No external command execution is introduced.
- Refactor does not change file-system permission handling.
- Input remains interpreted strictly as editor intent payloads.

## Configuration
- No new config options.
- Existing keybinding semantics remain unchanged.

## Component Interactions
1. Terminal key event enters active mode handler.
2. Mode handler resolves sequence via keymap.
3. Mode handler emits `HandleKeyResult::Complete(Intent)`.
4. Main loop processes intent queue.
5. Layout dispatches command/action appropriately.
6. UI state and editor state update as before.

## Platform Considerations
- Terminal key canonicalization behavior must remain unchanged across supported terminals.
- Ensure control-key combinations used for commands continue to map consistently after keymap payload refactor.