# Normalized Key Parsing - Technical Design

## Architecture Overview

This design refactors the key parsing mechanism to use normalized string representations (`canonical_string()`) instead of raw `(KeyCode, Modifiers)` tuple matching. The key architectural changes are:

1. **Mode Trait Enhancement**: The `Mode` trait is modified to support stateful key handling with a pending key buffer
2. **Keymap Introduction**: A `Keymap` trait that defines the mapping from normalized key strings to actions
3. **Stateful Modes**: Each mode maintains a `Vec<String>` buffer of normalized keys and a state indicator

### Data Flow

```
Raw Key Input → canonical_string() → Keymap Lookup → Action
                     ↓
              Buffer (if partial match)
```

### Key Architectural Decisions

- **Linear buffer approach**: For simplicity, use a linear `Vec<String>` to accumulate keys rather than a trie
- **Keymap-first lookup**: Always check if the current key sequence maps to an action before deciding to buffer
- **Escape clears buffer**: Escape key always resets the mode to idle state
- **No timeout**: Buffer persists until a conclusive match or escape

## Interface Design

### Mode Trait Changes

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `handle_key(&mut self, key: &Key)` | `&Key` | `HandleKeyResult` | Process key with internal state |
| `keymap(&self)` | - | `&dyn Keymap` | Get the keymap for this mode |
| `is_waiting(&self)` | - | `bool` | Whether mode is waiting for more keys |
| `clear_buffer(&mut self)` | - | `()` | Clear pending key buffer |

### Keymap Trait

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `get_action(&self, keys: &[String])` | `&[String]` | `Option<Action>` | Get action for key sequence |
| `is_prefix(&self, keys: &[String])` | `&[String]` | `bool` | Whether sequence could match longer binding |

### HandleKeyResult Enum

Instead of adding new variants to Action, create a new enum to represent the result of key handling:

```rust
/// Result of processing a key in a mode.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleKeyResult {
    /// A complete action is ready to execute.
    Complete(Action),
    /// Waiting for more keys to complete a sequence.
    WaitForMore,
    /// The key sequence was invalid or incomplete with no possible match.
    InvalidSequence,
}
```

## Data Models

### Keymap Trait

```rust
/// Trait for mapping normalized key sequences to actions.
pub trait Keymap {
    /// Get the action for a key sequence, if one exists.
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    
    /// Check if the given key sequence could be a prefix of a longer binding.
    fn is_prefix(&self, keys: &[String]) -> bool;
}
```

### SimpleKeymap Struct

```rust
/// A simple single-key keymap implementation using HashMap.
pub struct SimpleKeymap {
    bindings: HashMap<String, Action>,
}

impl SimpleKeymap {
    pub fn new() -> Self;
    pub fn insert(&mut self, key: String, action: Action);
}
```

## Key Components

### NormalMode

**Responsibilities:**
- Maintain a buffer of normalized keys
- Determine when to execute an action vs. wait for more input
- Clear buffer on Escape or invalid sequences

**Public API:**
- `new() -> NormalMode` - Create with default keymap
- `handle_key(&mut self, key: &Key) -> HandleKeyResult` - Process key with state
- `keymap(&self) -> &dyn Keymap` - Access the keymap
- `is_waiting(&self) -> bool` - Check if waiting for more keys
- `clear_buffer(&mut self) -> Vec<String>` - Clear and return buffered keys

**Internal State:**
- `keymap: SimpleKeymap` - Key to action mappings
- `buffer: Vec<String>` - Pending normalized keys
- `waiting: bool` - Whether waiting for more keys

### InsertMode

**Responsibilities:**
- Handle character insertion
- Support mode switching via Escape

**Public API:**
- Same as NormalMode
- Simplified keymap focused on character insertion

**Internal State:**
- Same structure as NormalMode
- Different keymap bindings

## User Interaction

### Key Handling Flow

1. User presses a key
2. Mode converts key to canonical string
3. Mode appends canonical string to buffer
4. Mode checks keymap for exact match:
   - **Exact match**: Execute action, clear buffer, return to idle → `HandleKeyResult::Complete(action)`
   - **Prefix match**: Set waiting = true → `HandleKeyResult::WaitForMore`
   - **No match**: Clear buffer → `HandleKeyResult::InvalidSequence`
5. If Escape is pressed at any time, clear buffer and return to idle → `HandleKeyResult::InvalidSequence`

### Example Flow: Single Key

```
User presses 'h' in NormalMode
→ canonical_string() = "h"
→ buffer = ["h"]
→ keymap.get_action(["h"]) = Some(Action::MoveLeft)
→ Clear buffer, return HandleKeyResult::Complete(Action::MoveLeft)
```

### Example Flow: Future Multi-Key (not implemented)

```
User presses 'd' (first of 'dd')
→ canonical_string() = "d"
→ buffer = ["d"]
→ keymap.get_action(["d"]) = None
→ keymap.is_prefix(["d"]) = true
→ Set waiting = true, return HandleKeyResult::WaitForMore

User presses 'd' again
→ canonical_string() = "d"
→ buffer = ["d", "d"]
→ keymap.get_action(["d", "d"]) = Some(Action::DeleteLine)
→ Clear buffer, return HandleKeyResult::Complete(Action::DeleteLine)
```

## External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| `Key::canonical_string()` | Convert keys to normalized strings | Already implemented (spec 0011) |
| `HashMap` | Efficient key lookup | Standard library |

## Error Handling

| Condition | Handling |
|-----------|----------|
| Key sequence has no match | Clear buffer, return `HandleKeyResult::InvalidSequence` |
| Key could be prefix but waiting | Stay in waiting state, return `HandleKeyResult::WaitForMore` |
| Escape pressed | Clear buffer, return to idle state |
| Invalid key (no canonical string) | Return `HandleKeyResult::InvalidSequence` |

## Security

No security concerns for this feature. Key parsing is internal to the application.

## Configuration

No new configuration options needed. Keybindings are defined in code via the keymap.

## Component Interactions

```
┌─────────────────────────────────────────────────────────┐
│                     Main Event Loop                     │
└─────────────────────┬───────────────────────────────────┘
                      │ key: Key
                      ▼
┌─────────────────────────────────────────────────────────┐
│                      Mode (trait)                       │
│  ┌─────────────────────────────────────────────────┐   │
│  │ buffer: Vec<String>                              │   │
│  │ waiting: bool                                    │   │
│  │ keymap: SimpleKeymap                             │   │
│  └─────────────────────────────────────────────────┘   │
│  handle_key(key) → canonical_string → keymap lookup    │
└─────────────────────┬───────────────────────────────────┘
                      │ HandleKeyResult
                      ▼
┌─────────────────────────────────────────────────────────┐
│                   Action Processor                      │
│   HandleKeyResult::Complete(Action) → execute          │
│   HandleKeyResult::WaitForMore → continue waiting       │
│   HandleKeyResult::InvalidSequence → ignore             │
└─────────────────────────────────────────────────────────┘
```

## Platform Considerations

No platform-specific considerations. This feature uses only standard Rust libraries.

## Trade-offs

**Decision**: Use linear buffer (`Vec<String>`) instead of trie for key sequence matching

**Reasoning**:
- Simpler to implement and understand
- Sufficient for initial single-key matching
- Can be extended to multi-key by checking prefix matches
- No performance concern for small number of keys

**Impact**:
- Multi-key lookup is O(n) where n is sequence length
- Will need different approach if hundreds of key sequences needed

**Decision**: No timeout for buffer clearing

**Reasoning**:
- User requested no timeout
- Keeps implementation simpler
- Vim doesn't have timeout for all sequences anyway

**Impact**:
- User may need to press Escape to cancel partial sequences
- No automatic recovery from stuck waiting state

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Buffer never clears on invalid key | Low | Medium | Escape always clears; implement AC-004 |
| Performance issue with many keymaps | Low | Low | HashMap provides O(1) single-key lookup |
| Backward compatibility broken | Low | High | Ensure AC-003 - tests must pass |

## Implementation Notes

### Backward Compatibility

The implementation must maintain exact backward compatibility:
- Single keys that previously mapped to actions must still map
- The main event loop needs to handle `HandleKeyResult`:
  - `HandleKeyResult::Complete(action)` → execute the action
  - `HandleKeyResult::WaitForMore` → continue waiting, don't process as action
  - `HandleKeyResult::InvalidSequence` → treat as no action (same as `Action::None`)

### Testing Strategy

- Unit tests for each mode's keymap
- Integration tests verifying key sequences produce correct actions
- Ensure all existing tests pass
- Test Escape clears buffer
- Test invalid keys clear buffer
