# Visual Scroll Margin - Technical Design

## Architecture Overview
The feature introduces a new configuration object, `scroll_margin`, that adjusts when viewport scrolling begins relative to cursor position. The viewport origin (`scroll_offset`) remains the same internal representation; only the trigger threshold for vertical and horizontal offset updates changes.

High-level flow:
1. Resolve `scroll_margin` from config with defaults.
2. During viewport reconciliation (`scroll_to_cursor`), compute effective visible rows/cols.
3. Clamp effective margins based on current viewport size.
4. Treat the viewport as having inner keep-zone bounds:
   - vertical: `[top + margin_v, bottom - margin_v]`
   - horizontal: `[left + margin_h, right - margin_h]`
5. If cursor moves outside keep-zone, shift `scroll_offset` enough to place cursor back on keep-zone boundary.
6. Clamp final offsets to buffer bounds as today.

## Interface Design
### Configuration interface
Add a new config table:

```rust
pub struct ScrollMargin {
    pub vertical: usize,
    pub horizontal: usize,
}

pub struct Config {
    pub scroll_margin: ScrollMargin,
    // existing fields...
}

pub struct PartialScrollMargin {
    pub vertical: Option<usize>,
    pub horizontal: Option<usize>,
}

pub struct PartialConfig {
    pub scroll_margin: Option<PartialScrollMargin>,
    // existing fields...
}
```

User-facing TOML shape:

```toml
scroll_margin = { vertical = 5, horizontal = 5 }
```

Behavior:
- Default values: `vertical = 5`, `horizontal = 5`.
- Missing keys in the table fall back to defaults.
- Unknown top-level or nested config keys remain rejected by existing strict parsing behavior.

### Scrolling interface
Keep `BufferView::scroll_to_cursor(viewport_size, gutter_width)` signature unchanged. Internally, update scroll trigger rules to use configured margins.

## Data Models
### ScrollMargin
- `vertical: usize`
  - Desired number of lines from top/bottom edge before vertical scrolling begins.
- `horizontal: usize`
  - Desired number of visual columns from left/right edge before horizontal scrolling begins.

Constraints:
- Non-negative integers.
- Effective values are runtime-clamped by viewport size.

### Effective margins
At runtime derive:
- `effective_vertical = min(config.vertical, max((visible_rows.saturating_sub(1)) / 2, 0))`
- `effective_horizontal = min(config.horizontal, max((visible_cols.saturating_sub(1)) / 2, 0))`

This guarantees at least one reachable cursor position in the keep-zone and avoids impossible constraints in tiny viewports.

## Key Components
### Config parsing and defaults (`src/config.rs`)
Responsibilities:
- add `ScrollMargin` and `PartialScrollMargin`
- merge partial values with defaults
- preserve strict unknown-field behavior
- expose resolved values globally via existing config access patterns

### Scroll reconciliation (`src/window/view.rs`)
Responsibilities:
- read resolved `scroll_margin` from global config
- compute visible rows/cols after gutter subtraction
- derive clamped effective margins
- update vertical/horizontal `scroll_offset` using keep-zone checks
- clamp offsets to max row/col bounds as currently done

### Docs (`docs/config.md`)
Responsibilities:
- add `scroll_margin` to schema list and sample config
- document defaults, behavior, and small-viewport clamping

## User Interaction
- User moves cursor with any supported movement path.
- Cursor can approach viewport edge while staying within configured margins.
- Once cursor enters an edge margin band, viewport scrolls to restore cursor to margin boundary.
- In very small windows, behavior gracefully falls back to tighter effective margins.

## External Dependencies
No new external dependencies.

## Error Handling
- Invalid `scroll_margin` TOML types continue to produce startup parse/validation errors.
- Unknown keys in `scroll_margin` are rejected by serde unknown-field handling.
- When viewport dimensions are zero or near zero, existing guard behavior and saturating arithmetic prevent panics.

## Security
No new security implications. The feature changes only in-memory scrolling behavior and startup configuration parsing.

## Configuration
New option:
- `scroll_margin` (table)
  - `vertical` (positive-or-zero integer, default `5`)
  - `horizontal` (positive-or-zero integer, default `5`)

Example:

```toml
scroll_margin = { vertical = 5, horizontal = 5 }
```

## Component Interactions
1. Config loader resolves `scroll_margin` into `Config`.
2. Editor stores resolved config globally.
3. Cursor movement updates cursor position through existing actions/motions.
4. Window render path invokes `scroll_to_cursor`.
5. `scroll_to_cursor` applies margin-aware trigger logic, then clamps offsets.
6. Render pipeline draws viewport from updated `scroll_offset`.

## Platform Considerations
- Behavior is terminal-size dependent by design and remains stable through runtime clamping.
- Horizontal behavior operates on visual columns and naturally respects tab-expanded widths through existing cursor visual-column logic.
