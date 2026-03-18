# gg and G Line Motions - Technical Design

## Architecture Overview

This feature adds two new line navigation motions to urvim:
- `gg` - Go to first line (or specified line with count)
- `G` - Go to last line (or specified line with count)

These motions integrate with the existing action system, keymap, and column preservation logic.

## Interface Design

### New Actions

| Action | Description |
|--------|-------------|
| `MoveToFirstLine` | Move cursor to first line (line 1), or to line N if count is provided (e.g., `5gg`) |
| `MoveToLastLine` | Move cursor to last line, or to line N if count is provided (e.g., `5G`) |

### Keymap Bindings

| Key Sequence | Action | Notes |
|--------------|--------|-------|
| `g` (first) | WaitForMore | Prefix for gg motion |
| `gg` | `MoveToFirstLine` | Go to first line |
| `G` | `MoveToLastLine` | Go to last line |

### Count Prefix Behavior

The count prefix works similarly to other motions:
- `gg` → goes to line 1
- `5gg` → goes to line 5
- `G` → goes to last line
- `5G` → goes to line 5

## Data Models

### Action Enum Changes

Add two new variants to the `Action` enum:

```rust
/// Move cursor to first line (or specified line with count)
MoveToFirstLine,
/// Move cursor to last line (or specified line with count)  
MoveToLastLine,
```

## Key Components

### 1. Action Methods Updates

Update the following methods in `Action`:

| Method | Update |
|--------|--------|
| `resets_remembered_column()` | Return `false` for `MoveToFirstLine` and `MoveToLastLine` (like vertical motions) |
| `uses_remembered_column()` | Return `true` for the new actions (vertical motion behavior) |
| `is_countable()` | Return `true` for the new actions (can be repeated) |
| `is_line_action()` | Return `true` for the new actions (count specifies target line) |

### 2. SimpleKeymap Multi-Key Support

Extend `SimpleKeymap` to store bindings as a single vector:

```rust
pub struct SimpleKeymap {
    // Vec of (key sequence, action) - supports both single keys and multi-key sequences
    // Lookups: check exact match or if current buffer is a prefix
    bindings: Vec<(Vec<String>, Action)>,
}

impl SimpleKeymap {
    /// Creates a new empty keymap.
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    /// Inserts a single-key binding.
    pub fn insert(&mut self, key: String, action: Action) {
        self.bindings.push((vec![key], action));
    }

    /// Inserts a multi-key sequence binding.
    pub fn insert_sequence(&mut self, keys: Vec<String>, action: Action) {
        self.bindings.push((keys, action));
    }
}

impl Keymap for SimpleKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action> {
        // Linear search - acceptable for small number of bindings
        self.bindings
            .iter()
            .find(|(binding_keys, _)| binding_keys == keys)
            .map(|(_, action)| action.clone())
    }

    fn is_prefix(&self, keys: &[String]) -> bool {
        // Check if any binding starts with the given keys
        self.bindings
            .iter()
            .any(|(binding_keys, _)| binding_keys.starts_with(keys))
    }
}
```

**Rationale:**
- Simpler data structure (single Vec instead of HashMap + separate sequences)
- Linear search is acceptable given small number of keybindings (~30)
- Easy to migrate to trie later when needed

### 3. NormalMode Updates

Update `NormalMode::new()` to register the new bindings:
```rust
// g as prefix key - will be handled specially in handle_key
keymap.insert("g".to_string(), Action::None);

// gg sequence  
keymap.insert_sequence(vec!["g".to_string(), "g".to_string()], Action::MoveToFirstLine);

// G key
keymap.insert("G".to_string(), Action::MoveToLastLine);
```

Update `handle_key` to handle the prefix key 'g':
- When 'g' is received and there's a pending count, wait for second key
- When second 'g' is received, execute `MoveToFirstLine` with count

### 4. Window Action Processing

Update `Window::process_action` to handle the new actions:

```rust
Action::MoveToFirstLine => {
    let target_line = if let Some(count) = pending_count {
        // Count provided: go to that line (1-indexed)
        (count - 1).min(self.buffer.line_count() - 1)
    } else {
        // No count: go to first line
        0
    };
    let target_col = self.buffer_view.get_or_compute_target_col();
    self.buffer_view.set_cursor(Cursor::new(target_line, target_col));
    ActionResult::Handled
}

Action::MoveToLastLine => {
    let target_line = if let Some(count) = pending_count {
        // Count provided: go to that line (1-indexed), clamped to last line
        (count - 1).min(self.buffer.line_count() - 1)
    } else {
        // No count: go to last line
        self.buffer.line_count() - 1
    };
    let target_col = self.buffer_view.get_or_compute_target_col();
    self.buffer_view.set_cursor(Cursor::new(target_line, target_col));
    ActionResult::Handled
}
```

**Important**: When these motions are used WITHOUT a count, they should:
1. Use the remembered column (via `get_or_compute_target_col()`)
2. Update the remembered column after moving (like MoveUp/MoveDown)

When used WITH a count, the count specifies the target line, so column handling is the same.

## User Interaction

### Normal Mode Usage

| Input | Result |
|-------|--------|
| `gg` | Move to first line |
| `5gg` | Move to line 5 |
| `G` | Move to last line |
| `5G` | Move to line 5 |

Note: Operator combinations (e.g., `dgg`, `dG`) are out of scope as operators are not yet implemented.

### Column Preservation

- After `gg` or `G`, subsequent vertical motions (`j`/`k`) use the column from before the motion
- This matches the behavior of `MoveUp` and `MoveDown`

## External Dependencies

No new external dependencies required.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Count exceeds line count | Clamp to last line (e.g., `999G` on a 100-line file goes to line 100) |
| Empty buffer | Both motions go to line 0 (cursor stays at start) |

## Trade-offs

**Decision**: Use multi-key sequence support in SimpleKeymap rather than special-casing in NormalMode

**Reasoning**:
- More extensible for future multi-key bindings (e.g., `z+`, `Ctrl-ww`)
- Cleaner separation of concerns
- Easier to test

**Impact**:
- Slightly more complex keymap implementation
- More memory for storing sequences

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Count parsing conflict with 'g' key | Low | Medium | Ensure 'g' alone doesn't conflict with other 'g'-based motions |
| Edge case: count 0 | Low | Low | Reject count 0 in `is_valid_count` (already done) |

## Implementation Plan

1. Add `MoveToFirstLine` and `MoveToLastLine` to `Action` enum
2. Update `Action` methods (`resets_remembered_column`, `uses_remembered_column`, `is_countable`, `is_line_action`)
3. Extend `SimpleKeymap` to support multi-key sequences
4. Register keybindings in `NormalMode::new()`
5. Handle the 'g' prefix in `NormalMode::handle_key()`
6. Add action handlers in `Window::process_action()`
7. Add unit tests
