# Character Scan Motions - Technical Design

## 2. Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│ NormalMode                                                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ ChainedKeymap                                                 ││
│  │  ┌─────────────────┐    ┌──────────────────────────────────┐ ││
│  │  │ TrieKeymap     │ OR │ CharScanKeymap (stateless)     │ ││
│  │  │ (fixed seqs)    │    │ (f/F/t/T trigger + char param) │ ││
│  │  └─────────────────┘    └──────────────────────────────────┘ ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Window::process_action()                                         │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ Action::FindForward(c)  → find_char_forward(c, count)      ││
│  │ Action::FindBackward(c) → find_char_backward(c, count)     ││
│  │ Action::TillForward(c)  → find_char_forward(c, count) - 1  ││
│  │ Action::TillBackward(c) → find_char_backward(c, count) + 1 ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow

1. User presses `f` → buffer becomes `["f"]` → `is_prefix(["f"])` → trie: false, char_scan: true → waiting
2. User presses `x` → buffer becomes `["f", "x"]` → `get_action(["f", "x"])` → char_scan returns `Action::FindForward('x')` → complete
3. `process_action(Action::FindForward('x'))` → searches for 'x' → moves cursor

### Key Architectural Decisions

- **Character scan keymap is stateless**: It only inspects the key buffer to determine if a valid trigger+target pair is present. No internal state to maintain.
- **Chained keymap tries trie first**: Fixed sequences (like `gg`, `dd`) take precedence over character scan triggers.
- **Two-key sequence matched programmatically**: Instead of storing 1024 entries (4 triggers × 256 chars), the char scan keymap matches patterns programmatically.

---

## 3. Interface Design

### Keymap Trait (existing)

```rust
pub trait Keymap {
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    fn is_prefix(&self, keys: &[String]) -> bool;
}
```

### CharScanKeymap

```rust
/// A stateless keymap for character scan motions (f, F, t, T).
///
/// Matches two-key sequences where:
/// - First key is a character scan trigger (f, F, t, T)
/// - Second key is any character (the target)
///
/// Returns `Action::FindForward(char)`, `Action::FindBackward(char)`,
/// `Action::TillForward(char)`, or `Action::TillBackward(char)`.
pub struct CharScanKeymap;

impl CharScanKeymap {
    pub fn new() -> Self;
}

impl Keymap for CharScanKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    fn is_prefix(&self, keys: &[String]) -> bool;
}
```

### ChainedKeymap

```rust
/// A keymap wrapper that chains multiple keymaps together.
///
/// Tries each sub-keymap in sequence until one returns a non-None result.
/// Order matters: first keymap in the chain has priority.
pub struct ChainedKeymap {
    keymaps: Vec<Box<dyn Keymap>>,
}

impl ChainedKeymap {
    pub fn new(keymaps: Vec<Box<dyn Keymap>>) -> Self;
    pub fn add(&mut self, keymap: Box<dyn Keymap>);
}

impl Keymap for ChainedKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    fn is_prefix(&self, keys: &[String]) -> bool;
}
```

### Action Enum (new variants)

```rust
pub enum Action {
    // ... existing variants ...

    /// Find forward: move cursor to the next occurrence of char
    FindForward(char),
    /// Find backward: move cursor to the previous occurrence of char
    FindBackward(char),
    /// Till forward: move cursor to the position before the next occurrence
    TillForward(char),
    /// Till backward: move cursor to the position after the previous occurrence
    TillBackward(char),
}
```

---

## 4. Data Models

### CharScanKeymap Matching Rules

| Keys Input | Trigger Key | Action Returned |
|------------|-------------|-----------------|
| `["f", "x"]` | f | `Some(Action::FindForward('x'))` |
| `["F", "x"]` | F | `Some(Action::FindBackward('x'))` |
| `["t", "x"]` | t | `Some(Action::TillForward('x'))` |
| `["T", "x"]` | T | `Some(Action::TillBackward('x'))` |
| `["f"]` | f | `None` (needs target) |
| `["g", "g"]` | - | `None` (not a char scan trigger) |
| Any other | - | `None` |

### is_prefix Rules for CharScanKeymap

| Keys Input | Result | Reason |
|------------|--------|--------|
| `["f"]` | `true` | Waiting for target char |
| `["F"]` | `true` | Waiting for target char |
| `["t"]` | `true` | Waiting for target char |
| `["T"]` | `true` | Waiting for target char |
| `["f", "x"]` | `false` | Complete two-key sequence |
| `["f", "x", "y"]` | `false` | Not a valid sequence |
| Any other | `false` | Not a char scan trigger |

### ChainedKeymap Behavior

| Keys | TrieResult | CharScanResult | Final Result |
|------|------------|----------------|--------------|
| `["g"]` | None | false | `false` |
| `["g", "g"]` | Some(MoveToFirstLine) | false | `Some(MoveToFirstLine)` |
| `["f"]` | false | true | `true` |
| `["f", "x"]` | None | Some(FindForward('x')) | `Some(FindForward('x'))` |

---

## 5. Key Components

### CharScanKeymap

**Responsibilities:**
- Detect character scan trigger keys (f, F, t, T)
- Match two-key sequences programmatically
- Return appropriate Action with target character

**Implementation Approach:**
```rust
fn get_action(&self, keys: &[String]) -> Option<Action> {
    if keys.len() != 2 {
        return None;
    }

    let [trigger, target] = keys else { return None };
    let target_char = target.chars().next()?;
    let key_str = trigger.as_str();

    match key_str {
        "f" => Some(Action::FindForward(target_char)),
        "F" => Some(Action::FindBackward(target_char)),
        "t" => Some(Action::TillForward(target_char)),
        "T" => Some(Action::TillBackward(target_char)),
        _ => None,
    }
}

fn is_prefix(&self, keys: &[String]) -> bool {
    keys.len() == 1 && matches!(keys[0].as_str(), "f" | "F" | "t" | "T")
}
```

**Dependencies:** None (stateless)

---

### ChainedKeymap

**Responsibilities:**
- Delegate to multiple keymaps in sequence
- Return first non-None result for get_action
- Return true if any sub-keymap returns true for is_prefix

**Implementation Approach:**
```rust
fn get_action(&self, keys: &[String]) -> Option<Action> {
    for keymap in &self.keymaps {
        if let Some(action) = keymap.get_action(keys) {
            return Some(action);
        }
    }
    None
}

fn is_prefix(&self, keys: &[String]) -> bool {
    for keymap in &self.keymaps {
        if keymap.is_prefix(keys) {
            return true;
        }
    }
    false
}
```

**Dependencies:** Any keymap implementing `Keymap` trait

---

### Window Motion Handlers

**Responsibilities:**
- Execute character scan actions by moving the cursor
- Implement forward/backward search with count support
- Handle "not found" case (stay in place)

**New methods in Window:**
```rust
fn move_cursor_to_char_forward(&mut self, target: char, count: usize);
fn move_cursor_to_char_backward(&mut self, target: char, count: usize);
```

**Search Algorithm (forward):**
```
1. Get current cursor position (line, col)
2. Get line content
3. Starting from col + 1, search forward for target char
4. If found and count > 1, continue searching for Nth occurrence
5. If found:
   - For FindForward: move to that position
   - For TillForward: move to position - 1
6. If not found: stay in place (do not move cursor)
```

**Dependencies:** Buffer access for line content

---

## 6. User Interaction

### Key Sequence Flow

```
Normal Mode:
  [no buffer] -- press 'f' --> buffer = ["f"], waiting = true
  buffer = ["f"] -- press 'x' --> buffer = ["f", "x"], execute FindForward('x')
  buffer = ["f", "x"] -- complete, clear buffer

Escape during wait:
  buffer = ["f"], waiting = true -- press Esc --> buffer = [], waiting = false
```

### Integration with Count Prefix

```
[no buffer] -- press '3' --> buffer = ["3"], waiting = true
buffer = ["3"] -- press 'f' --> buffer = ["3", "f"], wait for target
buffer = ["3", "f"] -- press 'x' --> execute Count(3, FindForward('x'))
```

---

## 7. External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| None | - | This feature is self-contained |

---

## 8. Error Handling

| Scenario | Behavior |
|----------|----------|
| Target char not found (forward) | Cursor stays in place |
| Target char not found (backward) | Cursor stays in place |
| TillForward lands before line start | Clamp to column 0 |
| TillBackward lands after line end | Clamp to last column |
| Count exceeds occurrences | Land on last available occurrence |
| Invalid key in sequence | Handled by mode (invalid sequence) |

---

## 9. Security

Not applicable - this is a local text editor with no network access or external data flow.

---

## 10. Configuration

No new configuration options required.

---

## 11. Component Interactions

### Flow: f x execution

```
User presses 'f'
  → NormalMode::handle_key(f)
  → keymap.is_prefix(["f"]) → trie: false, char_scan: true
  → waiting = true, return WaitForMore

User presses 'x'
  → NormalMode::handle_key(x)
  → keymap.get_action(["f", "x"]) → char_scan: Some(FindForward('x'))
  → return Complete(FindForward('x'))
  → Window::process_action(FindForward('x'))
  → Window::move_cursor_to_char_forward('x', 1)
  → cursor moved to 'x'
```

---

## 12. Platform Considerations

Not applicable - all components are platform-independent Rust code.

---

## 13. Trade-offs

**Decision**: Create a separate CharScanKeymap instead of extending TrieKeymap

**Reasoning**:
- Character scan motions have fundamentally different semantics (second key is a parameter, not a fixed binding)
- A separate keymap keeps the logic localized and makes the intent clear
- Chaining allows transparent composition without modifying existing TrieKeymap

**Alternative considered**: Modify TrieKeymap to support "any character" wildcards
- Rejected: Would complicate the trie structure and mixing of concerns

---

## 14. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Accidental keymap priority issue | Low | High | Test that `gg` is found before `g` in trie |
| Escape during char scan not canceling properly | Low | Medium | Verify NormalMode escape handling clears buffer |
| Till offset calculating wrong position | Medium | Medium | Comprehensive unit tests for all offset cases |

---

## 15. Testing Strategy

### Unit Tests

**CharScanKeymap:**
- `get_action(["f", "x"])` returns `FindForward('x')`
- `get_action(["F", "x"])` returns `FindBackward('x')`
- `get_action(["t", "x"])` returns `TillForward('x')`
- `get_action(["T", "x"])` returns `TillBackward('x')`
- `get_action(["f"])` returns `None`
- `get_action(["g", "g"])` returns `None`
- `is_prefix(["f"])` returns `true`
- `is_prefix(["F"])` returns `true`
- `is_prefix(["t"])` returns `true`
- `is_prefix(["T"])` returns `true`
- `is_prefix(["f", "x"])` returns `false`
- `is_prefix(["g"])` returns `false`

**ChainedKeymap:**
- Tries first keymap, falls back to second
- get_action returns first non-None
- is_prefix returns true if any returns true

**Window motion handlers:**
- Forward search finds correct position
- Backward search finds correct position
- Count works (3fx finds 3rd occurrence)
- Not found leaves cursor in place
- Till offset lands one position before/after

### Integration Tests

- `fx` navigation works in actual buffer
- `3f x` with count prefix works
- Escape cancels pending char scan
- Mode switching works during char scan wait
