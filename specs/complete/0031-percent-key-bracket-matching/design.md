# Percent Key Bracket Matching - Technical Design

## Architecture Overview

The `%` key feature integrates into the existing normal mode key handling system. When the user presses `%` in normal mode, the system will:
1. Check if the current character under the cursor is a bracket
2. If yes, search for the matching bracket (forward for opening, backward for closing)
3. Move the cursor to the matching bracket position

This feature follows the existing motion/key handler pattern in urvim.

## Interface Design

| Interface | Input | Output | Description |
|-----------|-------|--------|-------------|
| NormalModeKeyHandler::handle_percent_key | cursor: CursorPosition | Option<CursorPosition> | Returns new cursor position if jump occurs |

| Function | Input | Output | Description |
|----------|-------|--------|-------------|
| find_matching_bracket | buffer: &Buffer, cursor: CursorPosition, bracket: char | Option<CursorPosition> | Finds matching bracket position |

## Data Models

No new data models required. Uses existing:
- `CursorPosition` (line: usize, col: usize)
- `Buffer` (existing buffer interface with char access)

## Key Components

### PercentKeyHandler

**Responsibilities:**
- Detect if current character is a bracket
- Determine direction (forward for opening, backward for closing)
- Delegate to bracket matching algorithm

**Public API:**
- `handle(buffer: &Buffer, cursor: CursorPosition) -> Option<CursorPosition>`

**Algorithm:**
```
1. Get character at cursor position
2. If character is opening bracket ( (, [, { ):
   - Search forward from cursor+1 to find matching closing bracket
   - Handle nesting by counting bracket depth
   - Return position of matching bracket
3. If character is closing bracket ( ), ], } ):
   - Search backward from cursor-1 to find matching opening bracket
   - Handle nesting by counting bracket depth
   - Return position of matching bracket
4. If character is not a bracket:
   - Return None (no movement)
```

### BracketMatcher

**Responsibilities:**
- Find matching bracket position with correct nesting

**Public API:**
- `find_matching(buffer: &Buffer, start: CursorPosition, bracket: char) -> Option<CursorPosition>`

**Algorithm:**
```
For opening bracket (searching forward):
  depth = 0
  position = start + 1
  while position < buffer.end:
    char = buffer.char_at(position)
    if char == bracket:
      depth += 1
    else if char == matching_bracket(bracket):
      if depth == 0:
        return position
      depth -= 1
    position += 1
  return None (no match found)

For closing bracket (searching backward):
  depth = 0
  position = start - 1
  while position >= 0:
    char = buffer.char_at(position)
    if char == bracket:
      depth += 1
    else if char == matching_bracket(bracket):
      if depth == 0:
        return position
      depth -= 1
    position -= 1
  return None (no match found)
```

## User Interaction

- **Trigger**: Press `%` in normal mode
- **On bracket**: Cursor jumps to matching bracket
- **On non-bracket**: No action (silent fail)

## External Dependencies

- `Buffer` trait: Must provide `char_at(line, col)` method
- `CursorPosition` struct: Must provide line/col fields

## Error Handling

| Condition | Behavior |
|-----------|----------|
| No bracket at cursor | No movement, no error |
| No matching bracket found | No movement, no error |
| Cursor at buffer boundary | Handle gracefully, no panic |

## Security

Not applicable - this is a local text navigation feature with no security implications.

## Configuration

No configuration required for this feature.

## Component Interactions

```
User presses '%' in normal mode
    ↓
NormalModeKeyHandler receives key
    ↓
PercentKeyHandler::handle() called
    ↓
BracketMatcher finds matching position
    ↓
Cursor position updated
```

## Trade-offs

**Decision**: Use simple depth-counting algorithm over more sophisticated parsing

**Reasoning**:
- Simpler to implement and maintain
- Matches vim's behavior for the common cases
- Performance is acceptable for typical buffer sizes

**Impact**:
- Does not handle bracket pairs inside strings/comments specially (matches vim behavior)
- O(n) complexity where n is distance to match

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Large buffer causes slow matching | Low | Medium | O(n) is acceptable; vim has same behavior |
| Unmatched brackets cause hang | Low | Low | Algorithm terminates at buffer boundaries |
