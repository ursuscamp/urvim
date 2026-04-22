# Visual Text Wrapping - Technical Design

## Architecture Overview
Visual wrapping is introduced as a render projection layer over existing logical buffer lines.

The design preserves current edit and motion semantics by separating concerns:
- Logical model: buffer content, cursor movement, motions (`h/j/k/l/w/e/...`) remain unchanged.
- Visual model: renderer computes wrapped visual rows for each visible logical line when a window-level wrap toggle is enabled.

Core flow for a wrapped window:
1. Resolve available text width for the window.
2. Segment each visible logical line into visual row slices according to configured wrap mode (`hard`/`soft`) using grapheme display width.
3. Render first visual row with gutter line number; continuation rows with blank gutter number slot.
4. Project logical cursor location to wrapped visual row/column for cursor drawing.

## Interface Design

### Window Wrap State
Expose per-window wrap enablement as window state.

Proposed interface:
```rust
pub enum WrapMode {
    Hard,
    Soft,
}

pub struct WindowViewOptions {
    pub wrap_enabled: bool,
}

impl Window {
    pub fn wrap_enabled(&self) -> bool;
    pub fn toggle_wrap(&mut self);
}
```

Constraints:
- `wrap_enabled` is window-scoped, not buffer-scoped.
- New windows initialize `wrap_enabled = false`.

### Configuration Surface
Add startup configuration for wrap strategy.

Proposed interface:
```rust
pub struct Configuration {
    pub wrap_mode: WrapMode,
}
```

Constraints:
- Accepted values: `hard`, `soft`.
- Invalid value handling follows existing config validation path.
- Wrap mode is global configuration; enablement remains per-window.

### Rendering Contract
Introduce a wrapped-line planning API consumed by render paths.

Proposed interface:
```rust
pub struct WrappedSegment {
    pub start_col: usize,
    pub end_col: usize,
    pub visual_width: u16,
    pub is_continuation: bool,
}

pub fn plan_wrapped_segments(
    line: &str,
    max_width: u16,
    mode: WrapMode,
) -> Vec<WrappedSegment>;
```

Constraints:
- `start_col` and `end_col` must align to grapheme boundaries.
- Segment width must not exceed `max_width` except for degenerate width handling.
- `soft` mode must prefer the nearest prior word boundary; fallback to hard split.

### Cursor Projection Contract
Provide logical-to-visual mapping for wrapped windows.

Proposed interface:
```rust
pub struct VisualCursorPosition {
    pub visual_row_offset: usize,
    pub visual_col: u16,
}

pub fn project_cursor_on_wrapped_line(
    line: &str,
    logical_col: usize,
    max_width: u16,
    mode: WrapMode,
) -> VisualCursorPosition;
```

Constraints:
- Input `logical_col` is a valid logical cursor byte column already normalized by existing cursor sync.
- Output row/column reflects grapheme display width and wrap segments.

## Data Models

### New/Updated Types
- `WrapMode` (new enum):
  - `Hard`
  - `Soft`
- `Window` (updated):
  - add `wrap_enabled: bool`
- `Configuration` (updated):
  - add `wrap_mode: WrapMode` with default `Hard` (or existing-default decision recorded in config docs)
- `WrappedSegment` (new render helper type):
  - `start_col: usize` (grapheme boundary byte index)
  - `end_col: usize` (exclusive grapheme boundary byte index)
  - `visual_width: u16`
  - `is_continuation: bool`
- `VisualCursorPosition` (new helper type)
  - `visual_row_offset: usize`
  - `visual_col: u16`

### Invariants
- Segment boundaries never split a grapheme cluster.
- Segment ordering is strictly increasing and contiguous over the rendered logical line.
- Cursor projection on wrapped lines is consistent with segment plan used for rendering.

## Key Components

### 1. Wrap Planner
Responsibility:
- Convert a single logical line into visual segments for a given width and wrap mode.

Public API:
- `plan_wrapped_segments(...)`

Dependencies:
- Grapheme iteration utilities already used by cursor/motion logic.
- Display width calculator for graphemes.

Behavior notes:
- `Hard`: accumulate graphemes until next grapheme would exceed width, then break.
- `Soft`: track eligible break opportunities (word boundaries) while accumulating; if overflow occurs, break at last eligible boundary, otherwise hard break at current limit.

### 2. Wrapped Renderer Integration
Responsibility:
- Use wrap planner only when `window.wrap_enabled()` is true.
- Render first segment with line number and continuation segments with blank gutter number cell.

Public API:
- Existing window render entrypoints (updated behavior only).

Dependencies:
- Gutter render utilities.
- Existing line drawing pipeline.

### 3. Cursor Visual Projection
Responsibility:
- Compute cursor visual row offset and column from logical line/column under wrapping.

Public API:
- `project_cursor_on_wrapped_line(...)`

Dependencies:
- Same wrap segment planning logic to guarantee alignment between text rows and cursor position.

### 4. Input Action for Toggle
Responsibility:
- Bind `<C-w>w` to per-window wrap toggle action.

Public API:
- Action enum/update and mode input mapping.

Dependencies:
- Window selection/focus infrastructure and redraw invalidation.

### 5. Config Loader/Validator
Responsibility:
- Parse `wrap_mode` from config file and provide default.

Public API:
- Existing configuration loading path.

Dependencies:
- TOML config parser and config docs.

## User Interaction
- Default startup behavior remains unchanged (wrapping off for each window).
- Pressing `<C-w>w` toggles wrapping for focused window only.
- When toggled on:
  - long logical lines visually wrap within that window width,
  - gutter line number appears once for each logical line (first visual row only),
  - cursor appears on correct wrapped row while movement semantics remain logical.
- Switching windows can show same buffer wrapped in one window and unwrapped in another.

## External Dependencies
No new external crates are required if existing grapheme segmentation and width computation utilities are reused.

If current code lacks robust width utilities, evaluate adding a well-supported Unicode width crate as a follow-up; not required by default design.

## Error Handling
- Invalid config value for `wrap_mode`:
  - follow existing config validation pattern (startup error or fallback behavior as already established by project conventions).
- Very small or zero available text width:
  - renderer should avoid panic and produce stable output (e.g., minimal safe segmentation behavior).
- Mixed-width Unicode edge cases:
  - segment planner should degrade gracefully and keep grapheme-boundary safety.

## Security
No authentication, authorization, or secret handling changes.

Input validation considerations:
- Config parsing must validate enum values.
- Rendering path must guard against panics from malformed width assumptions.

## Configuration
Add a new config field:
- `wrap_mode = "hard" | "soft"`

Defaults and behavior:
- Default wrap mode: `hard`.
- Wrap mode determines algorithm when a window has wrapping enabled.
- Wrap enablement itself is runtime window state toggled via `<C-w>w`, default off.

Documentation updates required:
- `docs/config.md` for `wrap_mode`.
- `docs/motions.md` for `<C-w>w` toggle behavior and logical motion semantics under wrapping.

## Component Interactions

1. Startup:
- Config loader parses `wrap_mode` into `Configuration`.

2. Input handling:
- `<C-w>w` triggers wrap toggle action on focused window.
- Window state updates `wrap_enabled`; render invalidation requested.

3. Rendering:
- Window render path checks `wrap_enabled`.
- If off: existing render path.
- If on: wrap planner computes segments per visible logical line and draws rows with continuation gutter behavior.

4. Cursor drawing:
- Cursor render calls wrapped projection helper for wrapped window and line.
- Resulting visual row/col merged into viewport layout placement.

5. Motions:
- Motion engine unchanged; operates on logical lines/columns.
- Render projection reflects resulting logical cursor position.

## Platform Considerations
- Terminal cell width behavior can vary for some Unicode code points; planner should rely on existing project width utility for consistency across platforms.
- Split windows with differing widths naturally produce different wrap segmentation and must be handled independently per window instance.
- Resizing terminal/window should invalidate cached segment plans (if caching is added later) to avoid stale wraps.
