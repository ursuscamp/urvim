# Tab Group - Technical Design

## Architecture Overview
The tab-group feature adds one new root UI container above `Window`. A `TabGroup` owns an ordered list of `Window` instances, tracks the active tab index, and renders a one-row tab bar above the active window content.

The key architectural choice is to keep `Window` as the leaf editor surface. A tab is not a new editing primitive; it is a container for an existing window. That lets the feature reuse the current buffer editing, cursor, gutter, undo, and render logic with minimal behavioral change.

Runtime flow becomes:

1. `main` builds a `TabGroup` from the startup files.
2. The tab group renders the tab bar into the first terminal row.
3. The active `Window` renders into the remaining rows.
4. Key actions are routed to the active window unless they are tab-switch actions.

The tab bar maintains its own horizontal viewport so it can show only the visible subset of tabs. That viewport scrolls only when the active tab would otherwise be offscreen, and it does not shift when the selected tab is already visible.

## Interface Design

### TabGroup

```rust
pub struct TabGroup {
    tabs: Vec<Window>,
    active_tab: usize,
    tab_bar_start: usize,
}

impl TabGroup {
    pub fn new(tabs: Vec<Window>) -> Self;
    pub fn from_buffers(buffers: Vec<Buffer>) -> Self;

    pub fn active_tab_index(&self) -> usize;
    pub fn active_window(&self) -> &Window;
    pub fn active_window_mut(&mut self) -> &mut Window;
    pub fn active_buffer_view(&self) -> &BufferView;
    pub fn active_buffer_view_mut(&mut self) -> &mut BufferView;

    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size);
    pub fn visual_cursor(&self) -> Option<Position>;
}

impl Widget for TabGroup {
    fn process_action(&mut self, action: &Action) -> ActionResult;
}
```

### Action additions

Add two new editor actions:

```rust
pub enum Action {
    PreviousTab,
    NextTab,
    // existing variants...
}
```

These actions are only tab-navigation commands. They do not change mode, do not touch buffer snapshots, and do not affect undo history.

They are countable. When wrapped in `Action::Count`, the active tab moves repeatedly in the requested direction. For example, `3]b` advances three tabs to the right, wrapping as needed.

They should be treated as navigation-only actions in `Action` classification helpers. In particular, they are countable but do not reset remembered column state and do not switch the editor into insert mode.

### Key bindings

Normal mode maps:

- `[b` -> `Action::PreviousTab`
- `]b` -> `Action::NextTab`

The existing multi-key trie parser already supports these sequences, so no new key parser machinery is needed.

### Tab labels

Tab labels are derived from the active `Window`'s buffer metadata:

- If the buffer has a file name, render that name.
- Otherwise render `Untitled`.

Label generation is a pure display concern. It should not mutate the buffer or store separate persisted tab metadata for this first version.
Label width must be measured in terminal display cells, not bytes, so wide Unicode characters are handled correctly.

## Data Models

### TabGroup state

| Field | Type | Purpose |
|-------|------|---------|
| `tabs` | `Vec<Window>` | Ordered collection of open tabs |
| `active_tab` | `usize` | Index of the selected tab |
| `tab_bar_start` | `usize` | First tab index currently visible in the tab bar |

### Visibility model

The tab bar is a horizontal viewport over the tab list.

- The bar reserves up to one column on the left for a left arrow indicator.
- The bar reserves up to one column on the right for a right arrow indicator.
- The remaining columns are used for contiguous tab labels.
- A tab entry is considered visible only when its full label fits inside the current viewport, unless the terminal is too narrow to fit the full label, in which case the label is clipped.
- Tab widths are computed with the same Unicode-aware display width rules used by the rest of the editor.

The viewport is stateful. If the active tab is already visible, the viewport remains unchanged. The viewport only changes when the selected tab would otherwise fall outside the visible slice.

### Layout model

The terminal layout becomes:

| Row | Content |
|-----|---------|
| 0 | Tab bar |
| 1..n | Active window content |

If the terminal height is 0, nothing is rendered. If the terminal height is 1, only the tab bar is rendered and the active window receives a zero-height content area.

## Key Components

### TabGroup

**Responsibilities**
- Own the list of windows.
- Keep track of the active tab.
- Render the tab bar and route render size to the active window.
- Route editing actions to the active window.
- Handle tab navigation actions directly.

**Public behavior**
- `new` and `from_buffers` build the container from startup state.
- `process_action` switches tabs on `PreviousTab` / `NextTab`, including count-wrapped forms.
- `render` draws the bar and the active child window.
- `visual_cursor` returns the active window cursor offset by the bar height.

**Dependencies**
- `Window`, `BufferView`, `Screen`, `Widget`, `Action`

### Tab bar viewport logic

The scroll algorithm is intentionally conservative:

1. Build tab entries with their display widths.
2. Start from `tab_bar_start`.
3. Add tabs to the visible slice until the visible width is exhausted.
4. If the active tab is not fully inside the visible slice, shift `tab_bar_start` just enough to bring the active tab into view.
5. If the active tab is already visible, keep `tab_bar_start` unchanged.
6. Render arrow indicators only when there are hidden tabs on that side.

This rule matches the clarified behavior:
- moving to a tab offscreen to the right scrolls the bar so the tab becomes visible;
- moving back left to a tab that is still visible does not force a leftward shift.

Wrap-around tab navigation uses the same viewport rule. When tab selection wraps from one end to the other, the bar shifts only if the wrapped-to tab would otherwise be offscreen.

Counted tab navigation repeats the same movement multiple times and then applies the same visibility rule once for the final active tab. This keeps the bar stable during intermediate steps while still landing on the correct final tab.

### Startup file loading

`main` builds the initial `Vec<Window>` by iterating over all CLI paths:

1. Try to load each file into a `Buffer`.
2. On success, create a `Window` for that buffer and append it to the tab list.
3. On failure, log a warning and continue with the remaining paths.
4. If no file loads succeed, create one empty tab so the editor still starts in an editable state.
5. Select the first successfully loaded tab as active.

This keeps startup resilient while preserving the current "open whatever you can" behavior.

### Main loop integration

The main event loop continues to own mode switching and snapshot orchestration, but it asks the `TabGroup` for the active window/buffer view instead of talking to a single `Window`.

That means:
- undo/redo still operate on the active tab's buffer;
- snapshot capture still uses the active tab's cursor;
- insert/normal mode transitions remain application-level concerns;
- tab switching remains a widget-level concern.

## User Interaction

### Tab switching

- `[b` selects the previous tab.
- `]b` selects the next tab.
- Navigation wraps around at the ends of the tab list.

### Rendering

- The active tab is rendered with a distinct visual style in the tab bar.
- Hidden tabs do not disappear logically; they only move outside the visible bar viewport.
- Left and right arrow indicators appear when tabs are hidden offscreen on that side.
- The active window content always starts below the tab bar.

### Example scroll behavior

If the bar can currently show tabs 3 through 6:

- selecting tab 7 shifts the bar right so tab 7 becomes visible;
- selecting tab 6 does not move the bar if tab 6 is already visible;
- selecting tab 2 shifts the bar left so tab 2 becomes visible;
- selecting tab 3 while it is already visible does not move the bar.

## External Dependencies

No new crates are required.

The feature reuses:
- `clap` for CLI parsing
- existing `Buffer::load_from_file`
- existing `Screen` rendering and Unicode width handling
- existing `Widget` action routing

## Error Handling

| Scenario | Handling |
|----------|----------|
| Startup file fails to load | Log a warning and continue opening remaining files |
| No startup files succeed | Create one empty tab |
| Tab navigation at the end of the list | Wrap to the opposite end |
| Very narrow terminal width | Clip labels as needed; never panic |
| Active tab index becomes invalid due to a bug | Guard all index access and fall back to the first tab if necessary |

The implementation should treat tab navigation as a pure container operation. It must not mutate buffers, trigger snapshots, or change editor mode.

## Security

This feature has no new security-sensitive behavior. It only changes local terminal UI layout and startup file selection.

## Configuration

No new configuration is required.

Tab bar behavior is fixed for the first version:
- one row at the top
- horizontal scrolling based on the active tab
- wrap-around navigation
- arrow indicators for offscreen tabs

## Component Interactions

```text
main
  ├─ parse CLI files
  ├─ load buffers
  ├─ build TabGroup
  └─ event loop
       ├─ TabGroup.render(screen, origin, size)
       │    ├─ render tab bar on row 0
       │    └─ render active Window on rows 1..n
       ├─ mode.handle_key(key)
       ├─ TabGroup.process_action(action)
       └─ tab-aware snapshot / undo / redo through active buffer view
```

### Action flow

1. Normal mode returns an action.
2. If the action is `[b` or `]b`, `TabGroup` updates the active tab and the tab bar viewport.
3. If the action is a counted tab switch, `TabGroup` applies the switch repeatedly and then updates the viewport for the final active tab.
4. If the action is a regular edit or motion, `TabGroup` forwards it to the active `Window`.
5. Main still handles mode transitions, undo, redo, and snapshot bookkeeping.

### Cursor flow

1. Active `Window` computes its cursor position relative to its own content area.
2. `TabGroup::visual_cursor()` adds one row to account for the tab bar.
3. `main` places the terminal cursor using that adjusted position.

## Platform Considerations

### Terminal resize

The visible tab slice must be recalculated on every render so the tab bar adapts to width changes naturally. A resize may change whether arrows are needed and which tabs fit, but it must not change the active tab.

### Unicode and wide labels

File names and untitled labels may contain wide characters. The tab bar should use the same width-aware rendering utilities already used elsewhere in the editor so labels do not corrupt adjacent cells.

### Minimal height

When the terminal is too short to show both the tab bar and content, the tab bar still wins because it is the root navigation affordance. Content rendering then receives the remaining rows, which may be zero.
