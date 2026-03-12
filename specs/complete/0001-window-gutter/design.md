# Window Gutter - Technical Design

## Architecture Overview

The gutter feature adds a line number display to the left side of the editor window. It follows the same architectural pattern as Window itself: a dedicated struct with its own render method, owned by Window.

```
┌─────────────────────────────────────────────────────────────┐
│                        Terminal                              │
│  ┌─────────┬───────────────────────────────────────────────┐  │
│  │ Gutter  │              Window Content                  │
│  │ (3-6    │              (remaining cols)                │
│  │  cols)  │                                             │  │
│  │ ████    │                                             │  │
│  │  1      │  func main() {                              │  │
│  │  2      │      println!("Hello");                     │  │
│  │  3      │  }                                           │  │
│  │ ████    │                                             │  │
│  └─────────┴───────────────────────────────────────────────┘  │
```
(█ = gutter background fills all rows, not just content rows)

### Key Design Decisions

1. **No Buffer reference**: Gutter only needs `start_line`, `visible_rows`, and `total_buffer_lines` - no access to buffer content needed
2. **Full-height background**: Gutter renders background color for ALL visible rows (not just rows with content)
3. **Line wrapping preparation**: Track last rendered buffer line number; skip rendering if same line would repeat (blank cell for wrapped lines)
4. **Dynamic width**: Gutter width based on TOTAL buffer line count, not just visible lines (prevents width jitter when scrolling)
5. **No allocation for width**: Calculate digits mathematically without converting to string
6. **Buffer offset handling**: Content origin, size, and cursor all adjusted for gutter width

## Interface Design

### Gutter Struct

```rust
pub struct Gutter {
    /// First visible buffer line (scroll offset)
    start_line: usize,
    /// Number of visible rows in the window
    visible_rows: u16,
    /// Total number of lines in the buffer (for width calculation)
    total_buffer_lines: usize,
    /// Last rendered buffer line number (for wrapping detection)
    last_buffer_line: Option<usize>,
}

impl Gutter {
    /// Creates a new Gutter with viewport info
    /// 
    /// # Arguments
    /// * `start_line` - First visible buffer line (0-indexed)
    /// * `visible_rows` - Number of visible rows in window
    /// * `total_buffer_lines` - Total lines in buffer (for width calculation)
    pub fn new(start_line: usize, visible_rows: u16, total_buffer_lines: usize) -> Self;

    /// Calculates the required width for the gutter
    /// Width = digits(total_buffer_lines) + 2 (1 space padding each side)
    pub fn calculate_width(&self) -> u16;

    /// Renders the gutter to the screen at the given position
    /// Renders ALL visible_rows with background, line numbers for content rows
    pub fn render(&mut self, screen: &mut Screen, origin: Position);
}
```

### Window Integration

```rust
impl Window {
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let buffer = self.buffer_view.buffer();
        let total_lines = buffer.line_count();
        let start_line = self.buffer_view.scroll_offset().row as usize;

        // Create gutter with needed info (no buffer reference)
        let mut gutter = Gutter::new(
            start_line,
            size.rows,
            total_lines,
        );
        let gutter_width = gutter.calculate_width();

        // Render gutter at origin position
        gutter.render(screen, origin);

        // Render buffer content offset by gutter width
        let content_origin = Position::new(origin.row, origin.col + gutter_width);
        let content_size = Size::new(size.rows, size.cols.saturating_sub(gutter_width));
        self.render_data.render(screen, content_origin);
    }
}
```

## Data Models

### Gutter Fields

| Field | Type | Description |
|-------|------|-------------|
| start_line | usize | First visible buffer line (scroll offset) |
| visible_rows | u16 | Number of visible rows |
| total_buffer_lines | usize | Total lines in buffer for width calc |
| last_buffer_line | Option<usize> | Last rendered line (for wrapping detection) |

### Gutter Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| background_color | Color | ANSI 236 | Background color for gutter |
| foreground_color | Color | ANSI 245 | Text color for line numbers |
| padding_left | u16 | 1 | Space before line number |
| padding_right | u16 | 1 | Space after line number |

### Width Calculation

```
gutter_width = digits(total_buffer_lines) + padding_left + padding_right
            = digits(total_buffer_lines) + 2
```

**Mathematical digit counting (no string allocation):**

```rust
fn digit_count(n: usize) -> usize {
    if n == 0 { return 1; }
    let mut count = 0;
    let mut n = n;
    while n > 0 {
        count += 1;
        n /= 10;
    }
    count
}
```

Or more concisely using `ilog10` (Rust 1.67+):
```rust
fn digit_count(n: usize) -> usize {
    if n == 0 { return 1; }
    n.ilog10() as usize + 1
}
```

Examples (assuming scrolling to start of buffer):
- Buffer 1-9 lines (1 digit): width = 1 + 2 = 3
- Buffer 1-99 lines (2 digits): width = 2 + 2 = 4
- Buffer 1-999 lines (3 digits): width = 3 + 2 = 5
- Buffer 1-9999 lines (4 digits): width = 4 + 2 = 6

Note: Width uses TOTAL buffer lines, not just visible. This prevents gutter width from jumping around when scrolling between pages.

## Key Components

### Gutter

**Responsibilities:**
- Calculate required width based on total buffer lines
- Render line numbers with correct alignment and styling
- Render background for ALL visible rows
- Skip rendering line number if same buffer line would repeat (wrapping support)

**Public API:**
- `new(start_line: usize, visible_rows: u16, total_buffer_lines: usize) -> Gutter`
- `calculate_width() -> u16`
- `render(screen: &mut Screen, origin: Position)`

**Dependencies:**
- Screen (for rendering)
- Style, Color (for styling)

### Window

**Responsibilities:**
- Own and coordinate gutter rendering
- Pass required info to Gutter (start_line, visible_rows, total_lines)
- Adjust content origin by gutter width when rendering
- Adjust content size by gutter width (subtract from available columns)
- Adjust visual cursor position by gutter width

**Modified:**
- `render` method now creates Gutter, offsets content origin and size
- `visual_cursor` method now accounts for gutter width

## User Interaction

The gutter is transparent to users - it renders automatically as part of the window. No direct user interaction with gutter.

## External Dependencies

No external dependencies. Uses existing types:
- `Screen` from screen module
- `Style`, `Color` from terminal::style

## Error Handling

| Scenario | Handling |
|----------|----------|
| Empty buffer (0 lines) | Use width for 1 digit (minimum 3 columns) |
| Buffer with lines 1-9 | Use 3 column gutter (1 digit + 2 padding) |
| Buffer with lines > visible | Width based on total, not just visible |

## Security

Not applicable - this is a display-only feature with no security implications.

## Configuration

For this initial implementation, we use hardcoded defaults with no configuration.

## Component Interactions

```
Window::render()
    │
    ├─► Get buffer info: start_line, visible_rows, total_lines
    │
    ├─► Gutter::new(start_line, visible_rows, total_lines)
    │       │
    │       └─> calculate_width() → u16
    │
    ├─► Gutter::render(screen, origin)
    │       │
    │       ├─► For each visible row:
    │       │   ├─► Write background color to full gutter width
    │       │   ├─► If buffer_line != last_buffer_line:
    │       │   │   └─► Write line number (right-aligned)
    │       │   └─► Update last_buffer_line
    │       │
    │       └─► (Background written for ALL rows)
    │
    ├─► Calculate content_origin = origin + (0, gutter_width)
    │
    ├─► Calculate content_size = (rows, cols - gutter_width)
    │
    └─► RenderData::render(screen, content_origin)


Window::visual_cursor()
    │
    ├─► Get cursor position from render_data
    │
    ├─► Get gutter width
    │
    └─► Return cursor + (0, gutter_width)
```

## Line Wrapping Preparation

The gutter tracks `last_buffer_line` to detect when a buffer line wraps to the next screen row:

```
Screen row 0: buffer line 5  → render "5"
Screen row 1: buffer line 5  → render "   " (blank, same as above - wrapped)
Screen row 2: buffer line 6  → render "6"
```

This ensures that when line wrapping is added later, wrapped lines won't show redundant line numbers.

## Buffer Offset and Cursor Adjustment

When gutter is present, it consumes screen columns that were previously available for buffer content. Three adjustments are needed:

### 1. Buffer Render Origin Offset

The buffer content starts after the gutter:

```rust
let content_origin = Position::new(origin.row, origin.col + gutter_width);
```

### 2. Buffer Render Size Reduction

The buffer has fewer columns available:

```rust
let content_size = Size::new(size.rows, size.cols.saturating_sub(gutter_width));
```

### 3. Visual Cursor Position Adjustment

The cursor's visual position must account for the gutter:

```rust
impl Window {
    pub fn visual_cursor(&self) -> Option<Position> {
        // Get cursor position from render data
        if let Some(mut pos) = self.render_data.cursor_screen_position(self.buffer_view.cursor()) {
            // Add gutter width to column to get screen position
            let gutter = Gutter::new(
                self.buffer_view.scroll_offset().row as usize,
                self.render_data.visible_rows(),
                self.buffer_view.buffer().line_count(),
            );
            pos.col += gutter.calculate_width();
            Some(pos)
        } else {
            None
        }
    }
}
```

### Rationale

- **Origin**: The gutter occupies columns 0 to gutter_width-1, so content starts at gutter_width
- **Size**: Buffer can only render in columns gutter_width to size.cols-1, giving (size.cols - gutter_width) available columns
- **Cursor**: Cursor is calculated relative to content origin, so we add gutter width to get absolute screen position

## Trade-offs

**Decision**: No Buffer reference in Gutter, pass primitives instead

**Reasoning:**
- Cleaner separation of concerns - Gutter only needs numbers, not content
- Easier to test - no need for full Buffer mock
- Matches user's feedback: "doesn't need to access the buffer content"

**Impact:**
- Caller must provide total_buffer_lines (can get from RenderData or Buffer)

**Decision**: Width based on TOTAL buffer lines, not visible

**Reasoning:**
- Prevents gutter width from jumping as user scrolls
- Matches vim behavior - gutter width is stable

**Impact:**
- Large files (>1000 lines) will have wider gutter even when viewing top

**Decision**: Track last_buffer_line for wrapping support

**Reasoning:**
- Future-proofs for line wrapping
- No extra cost - we have the info anyway when iterating

**Impact:**
- Slight complexity in render logic, but minimal

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Gutter width calculation incorrect | Low | Medium | Unit tests for width calculation |
| Background not filling all rows | Low | High | Test with windows larger than content |
| Wrapping detection logic wrong | Low | Medium | Unit tests for repeated line numbers |
| Performance with very large files | Low | Low | Only O(visible_rows), width calc is O(1) |
| Cursor rendered in wrong column | Low | High | Test cursor position with gutter present |
| Content overflow when gutter added | Low | High | Verify content doesn't wrap unexpectedly |
