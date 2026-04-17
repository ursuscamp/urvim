# Pane Resizing - Technical Design

## Architecture Overview
Pane resizing will be implemented as a dedicated editor mode that lives alongside the existing normal, insert, and visual modes. Normal mode will gain a `Ctrl-w r` binding that switches the focused window into resizing mode. While resizing mode is active, the mode will translate `h`, `j`, `k`, and `l` into resize actions and translate `Esc` back to normal mode.

The layout layer will own the actual resize behavior. When it receives a resize action, it will update the split weights on the nearest ancestor split that matches the requested axis and contains the focused pane. Rendering already derives pane regions from split weights, so no separate persisted size model is needed.

## Interface Design

### Mode changes
- Add `ModeKind::Resizing` with a user-facing label such as `RESIZE`.
- Add a new `ResizingMode` implementation under the editor module, following the same shape as the existing mode implementations.
- Extend `Window::switch_mode` so it can construct `ResizingMode` and report `ModeKind::Resizing` to the status bar.

### Actions
- Add resize actions to `ActionKind` for the four directional resize intents:
  - horizontal decrease
  - horizontal increase
  - vertical decrease
  - vertical increase
- Keep the actions shallow and intent-based so the layout layer remains responsible for interpreting them against the current tree.

### Key handling contract
- `NormalMode` will bind `<C-w>r` to `Action::mode_transition(ModeKind::Resizing)`.
- `ResizingMode` will bind:
  - `h` to horizontal resize toward the left
  - `l` to horizontal resize toward the right
  - `j` to vertical resize downward
  - `k` to vertical resize upward
  - `<Esc>` to `Action::mode_transition(ModeKind::Normal)`
- Any other key in resizing mode will return `HandleKeyResult::InvalidSequence`, which the main loop already ignores.

## Data Models

### `ModeKind`
Add a new `Resizing` variant so the active mode can be displayed in the footer and used for mode switching.

### Resize intent
Represent resize intent as a small enum or equivalent action payload describing:
- axis: horizontal or vertical
- direction: toward the first child or the second child on that axis

The resize intent does not need to persist across sessions. It only exists long enough to route a keypress into the layout tree.

### Split sizing
Reuse the existing `SplitSize` weight pair on each split node. Resizing will mutate those weights in place, which keeps render-time region calculation unchanged.

## Key Components

### `src/editor/normal.rs`
Owns the `<C-w>` prefix binding table. It will gain the `r` subcommand that transitions into resizing mode.

### `src/editor/resizing.rs`
New mode implementation responsible for:
- mapping resize keys to actions
- exiting on `Esc`
- ignoring unrelated keys without leaving the mode

### `src/editor/mode.rs`
Will gain the new `ModeKind` variant and its display label.

### `src/window/mod.rs`
Will construct the resizing mode and preserve the current window-local mode-switch behavior. No new state container is needed outside the existing boxed `Mode`.

### `src/layout/mod.rs` and `src/layout/tree.rs`
Will receive the resize actions and apply them to the split tree. The tree helpers will locate the nearest matching split ancestor for the focused pane, adjust that split’s weights, and keep the focused pane stable.

### `src/layout/render.rs`
Will continue to derive pane regions from split weights. No render-path special case is needed beyond the already existing layout redraw flow.

## User Interaction
1. The user presses `Ctrl-w r` while in normal mode.
2. The active window switches to resizing mode and the status bar label changes to `RESIZE`.
3. The user presses `h`, `j`, `k`, or `l`.
4. The resizing mode emits the corresponding resize action.
5. The main loop dispatches the action to the layout.
6. The layout updates the nearest matching split weights and redraws the tree.
7. The user presses `Esc` to return to normal mode.

If the user presses any unrelated key while resizing, the key is ignored and the mode stays active.

## External Dependencies
No new external crates or services are required. The feature stays inside the existing modal input, layout tree, and rendering stack.

## Error Handling
- If the focused pane has no ancestor split on the requested axis, the resize action should be treated as a harmless no-op.
- If a resize would make one side of the split smaller than the minimum usable size, the resize must clamp at that boundary and stop shrinking further.
- If the layout has already been pruned down to no panes, resize handling should not attempt to mutate the tree.
- Ignored keys in resizing mode should not leak to normal-mode or window-level command handling.

## Security
The feature does not introduce new trust boundaries, input sources, or secrets handling. It only changes in-memory UI state in response to keyboard input.

## Configuration
No new configuration options are required. Resizing mode is always available through `Ctrl-w r`, and its keybindings are fixed.

## Component Interactions
1. `NormalMode` parses `Ctrl-w r` and returns a mode transition to `ModeKind::Resizing`.
2. `Window::switch_mode` swaps in `ResizingMode`.
3. `ResizingMode::handle_key` maps directional keys to resize actions and `Esc` back to normal mode.
4. The main event loop dispatches resize actions to `Layout`.
5. `Layout` locates the focused pane’s nearest matching split ancestor and adjusts that split’s `SplitSize`.
6. `Layout::render` recomputes pane regions from the updated tree and the UI redraw reflects the new proportions.

## Platform Considerations
The behavior is terminal-agnostic because the resize math happens against the layout tree, not against any platform-specific UI primitive. The minimum-size clamp should account for small terminal sizes so panes do not collapse below a usable region when the screen is tight.
