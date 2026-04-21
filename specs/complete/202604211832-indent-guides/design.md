# Indent Guides - Technical Design

## Architecture Overview
Indent guides are a render-time overlay derived from the current cursor line, cursor visual column, and cached indent scopes. The feature uses a single active guide model instead of rendering all scopes.

The high-level flow is:
1. Resolve whether `indent_guides` is enabled in effective configuration.
2. Gather cursor line, cursor visual column, and containing scope ids from the indent scope cache.
3. Select the deepest scope whose indent visual column is less than or equal to the cursor visual column.
4. Convert the selected scope into an interior draw range (open+1 through close-1).
5. Render a vertical guide glyph at the selected scope column on each interior line.
6. Choose ASCII `|` unless `unicode_indent` capability is available, in which case reuse the existing Unicode line style.

## Interface Design
### Configuration
Extend the user configuration model with:

```rust
pub struct Config {
    pub indent_guides: bool,
    // ...existing fields
}
```

Behavior:
- Default is `true` when the field is omitted.
- Unknown or invalid values continue to follow existing config validation behavior.

### Active guide selection API
Add a small helper in the window/view or render support layer:

```rust
pub struct ActiveIndentGuide {
    pub column: usize,
    pub start_line_exclusive: usize,
    pub end_line_exclusive: usize,
}

pub fn active_indent_guide_for_cursor(
    buffer: &Buffer,
    cursor: Cursor,
) -> Option<ActiveIndentGuide>;
```

Behavior:
- Returns `None` when no eligible scope exists.
- Returns `None` when `start_line_exclusive + 1 > end_line_exclusive - 1` (no interior rows).
- Uses cached per-line containing scopes to avoid full recomputation.

### Render integration
Add a render helper:

```rust
pub fn indent_guide_glyph(capabilities: &TerminalCapabilities, line_style: &LineStyle) -> char;
```

Behavior:
- Returns `|` when `unicode_indent` is unavailable.
- Returns the existing Unicode vertical line glyph path when available.

## Data Models
### ActiveIndentGuide
- `column: usize`: visual column for the guide.
- `start_line_exclusive: usize`: opening boundary line index.
- `end_line_exclusive: usize`: closing boundary line index.

Constraints:
- Render lines are `(start_line_exclusive + 1)..(end_line_exclusive)`.
- Opening and closing lines are never rendered over.

### Scope metadata usage
Use existing `IndentScope` fields from cache:
- opening line
- closing line
- normalized visual indent width

No new persisted buffer data is required.

## Key Components
### Configuration parser and defaults
Responsibilities:
- add `indent_guides` to config schema/model
- ensure default `true`
- expose value to window rendering code
- document option in `docs/config.md`

### Cursor-scope selection helper
Responsibilities:
- read containing scope ids for cursor line
- map ids to scope records
- filter scopes by `scope_indent_column <= cursor_visual_column`
- choose deepest eligible scope
- derive interior draw range and reject empty interiors

### Render overlay path
Responsibilities:
- skip rendering when config is disabled
- skip when no active guide exists
- place guide glyph at selected column only on interior lines
- keep guide continuous across blank lines
- avoid replacing opening/closing line content

### Capability-aware glyph selection
Responsibilities:
- choose ASCII `|` fallback
- choose Unicode vertical line glyph from existing style when capability permits
- keep behavior consistent with existing split-border/line-drawing style

## User Interaction
- User moves cursor to a line/column inside nested indentation.
- If enabled and eligible, one vertical guide appears at the active scope column.
- Guide extends through interior lines, including blank lines.
- Guide stops immediately before the first shallower-indent line.
- If no eligible scope exists or there are no interior lines, no guide is shown.

## External Dependencies
No new external crates.

Relies on:
- existing indent scope cache
- existing terminal capability detection (`unicode_indent`)
- existing line-drawing style utilities for Unicode borders/guides

## Error Handling
Fail-open behavior:
- If cache data is unavailable/stale at render time, render without guide for that frame.
- If selected guide column is outside a line's drawable width, skip that cell without panicking.
- If capability state is unavailable, default to ASCII `|`.

## Security
No new security implications.

The feature only changes transient rendering output and does not alter file content or external I/O.

## Configuration
New option:
- `indent_guides` (`bool`, default `true`): enables rendering of cursor-active indent guide.

Capability interaction:
- `unicode_indent` present: use existing Unicode line style glyph.
- `unicode_indent` absent: use ASCII `|`.

Documentation updates:
- add the option to `docs/config.md`

## Component Interactions
1. Render loop starts frame for active window.
2. Render checks effective config `indent_guides`.
3. If enabled, render asks buffer/cache for cursor line containing scope ids.
4. Selection helper chooses deepest eligible scope at/before cursor visual column.
5. Helper returns interior draw range and guide column.
6. Render picks glyph based on terminal capabilities.
7. Render overlays the glyph in guide column for each interior line.

## Platform Considerations
- ASCII fallback ensures compatibility with basic terminals.
- Unicode mode behavior follows existing capability-gated line-drawing conventions for consistent appearance.
- Visual-column scope resolution keeps tabs and mixed indentation consistent across platforms.
