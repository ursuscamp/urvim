# Trie-Based Key Parser with Multiplicative Counts - Technical Design

## 2. Architecture Overview

This design refactors the key parsing system from a linear-search Vec-based implementation to a trie data structure, while adding support for multiplicative sub-counts.

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        NormalMode                                │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                     TrieKeymap                               │ │
│  │  ┌─────┐    ┌─────┐    ┌─────┐    ┌─────┐                 │ │
│  │  │ "g" │───▶│ "g" │    │ "j" │    │ "d" │                 │ │
│  │  └─────┘    └─────┘    └─────┘    └─────┘                 │ │
│  │                    │                │                       │ │
│  │              Action:           ┌─────┴─────┐                │ │
│  │              MoveToFirstLine   │ "d"       │                │ │
│  │                               └─────┘       │                │ │
│  │                               Action:      │                │ │
│  │                               DeleteLine ×2│                │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                              │                                    │
│                              ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │              CountParser                                      │ │
│  │  - Parses key sequences to extract action keys + count     │ │
│  │  - Handles multi-digit counts: "55" → 55                    │ │
│  │  - Handles sub-counts: "d5d" → ["d","d"], count=5          │ │
│  │  - Multiplies all sub-counts: "2d3d" → count=6             │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow

1. User presses keys → `NormalMode::handle_key()`
2. Keys accumulated in buffer
3. On each key, check if buffer could match a prefix in trie
4. When complete sequence received:
   - Extract action keys and count via `CountParser`
   - Look up action in trie using action keys
   - Wrap action with `Action::Count(count, Box::new(action))`
5. Return `HandleKeyResult::Complete(action)`

### Key Architectural Decisions

1. **Trie over Vec**: O(k) lookup instead of O(n×k) linear search
2. **Count parsing separated**: The trie handles key sequence matching, a separate `CountParser` handles count extraction
3. **Multiplicative counts**: All sub-counts multiply together (not additive)
4. **Backward compatible**: Existing single-count-prefix behavior preserved

## 3. Interface Design

### New Types

```rust
/// Parser that extracts action keys and multiplicative count from key sequences.
pub struct CountParser;

impl CountParser {
    /// Parse a key sequence to extract action keys and total count.
    /// 
    /// Returns (action_keys, total_count) where:
    /// - action_keys: The keys that form the actual keybinding (counts removed)
    /// - total_count: The multiplicative product of all count components (always >= 1)
    /// 
    /// Note: If total_count is 1, callers should NOT wrap with Action::Count
    /// (optimization to avoid unnecessary boxing).
    pub fn parse(keys: &[String]) -> (Vec<String>, usize) { ... }

    /// Check if a key string is a count digit (1-9).
    /// Note: "0" is NOT a count digit (it's MoveToLineStart).
    pub fn is_count_digit(s: &str) -> bool { ... }
}
```

### Modified Types

```rust
/// Trait for mapping normalized key sequences to actions.
pub trait Keymap {
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    fn is_prefix(&self, keys: &[String]) -> bool;
}

/// Trie-based keymap implementation (replaces SimpleKeymap).
pub struct TrieKeymap {
    root: TrieNode,
}

impl TrieKeymap {
    pub fn new() -> Self;
    pub fn insert(&mut self, key: String, action: Action);
    pub fn insert_sequence(&mut self, keys: Vec<String>, action: Action);
}

impl Keymap for TrieKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    fn is_prefix(&self, keys: &[String]) -> bool;
}

impl Default for TrieKeymap { ... }
```

### Mode Handler Changes

```rust
impl NormalMode {
    // Modified method signature - same interface, different implementation
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        // 1. Convert key to canonical string
        // 2. Add to buffer
        // 3. Check for count + action using CountParser + TrieKeymap
        // 4. Only wrap with Action::Count if count > 1 (optimization)
        // 5. Return appropriate result
    }
}
```

## 4. Data Models

### TrieNode

```rust
/// A node in the trie, representing a partial key sequence.
struct TrieNode {
    /// Child nodes keyed by the next key in the sequence.
    /// Using BTreeMap for deterministic iteration order (useful for prefix checks).
    children: std::collections::BTreeMap<String, TrieNode>,
    /// Action associated with this complete key sequence (if any).
    action: Option<Action>,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            action: None,
        }
    }
}
```

### TrieKeymap

```rust
/// Trie-based keymap for efficient key sequence matching.
/// 
/// Time complexity:
/// - get_action: O(k) where k = key sequence length
/// - is_prefix: O(k) where k = key sequence length
/// 
/// Space complexity: O(m) where m = total characters in all key sequences
pub struct TrieKeymap {
    root: TrieNode,
    /// Cache of action keys -> action for quick lookup after count extraction.
    /// This is a performance optimization for the common case.
    action_cache: HashMap<Vec<String>, Action>,
}
```

## 5. Key Components

### CountParser

**Responsibilities:**
- Parse key sequences to extract action keys (non-count keys)
- Calculate total multiplicative count from all count components
- Handle special case of "0" as MoveToLineStart

**Public API:**
- `CountParser::parse(keys: &[String]) -> (Vec<String>, usize)`
- `CountParser::is_count_digit(s: &str) -> bool`

**Algorithm:**
```
For each key in sequence:
    If key is a digit (1-9):
        If we haven't seen an action yet:
            Accumulate as leading multi-digit count
        Else:
            Start new sub-count (multiply accumulated, add new digit)
    Else (key is an action):
        If there's a current count:
            Multiply it into total_count
        Add key to action_keys
        Mark that we've seen an action

At end:
    If there's remaining current_count, multiply into total_count
```

### TrieKeymap

**Responsibilities:**
- Store key bindings in trie structure
- O(k) lookup of key sequences
- Support prefix checking for timeout handling

**Public API:**
- `new() -> TrieKeymap`
- `insert(key: String, action: Action)` - single key binding
- `insert_sequence(keys: Vec<String>, action: Action)` - multi-key binding
- `get_action(keys: &[String]) -> Option<Action>` - exact match lookup
- `is_prefix(keys: &[String]) -> bool` - prefix check

**Insert Algorithm:**
```
Start at root
For each key in sequence:
    If key not in current node's children:
        Create new child node
    Move to child node
At final node:
    Set action
```

**Lookup Algorithm:**
```
Start at root
For each key in sequence:
    If key not in current node's children:
        Return None
    Move to child node
Return node's action (may be None for incomplete sequences)
```

## 6. User Interaction

### Key Sequences and Counts

| Input | Buffer State | Action Keys | Count | Final Action |
|-------|--------------|-------------|-------|--------------|
| `j` | `["j"]` | `["j"]` | 1 | MoveDown (no wrap) |
| `5` | `["5"]` | - | Wait | WaitForMore |
| `5j` | `["5", "j"]` | `["j"]` | 5 | MoveDown wrapped in Count |
| `5dd` | `["5", "d", "d"]` | `["d", "d"]` | 5 | DeleteLine wrapped in Count |
| `d5d` | `["d", "5", "d"]` | `["d", "d"]` | 5 | DeleteLine wrapped in Count |
| `2d2d` | `["2", "d", "2", "d"]` | `["d", "d"]` | 4 | DeleteLine wrapped in Count |
| `12d34d` | `["1","2","d","3","4","d"]` | `["d", "d"]` | 408 | DeleteLine wrapped in Count |
| `0` | `["0"]` | `["0"]` | 1 | MoveToLineStart (no wrap) |
| `gg` | `["g", "g"]` | `["g", "g"]` | 1 | MoveToFirstLine (no wrap) |
| `5gg` | `["5", "g", "g"]` | `["g", "g"]` | 5 | MoveToFirstLine wrapped in Count |

### Interaction Flow

```
User types 'd':
  Buffer: ["d"]
  Check: is_prefix(["d"])? → true (for "dd", "dw", etc.)
  Result: WaitForMore

User types 'd' again:
  Buffer: ["d", "d"]
  Parse: action_keys=["d","d"], count=1
  Lookup: get_action(["d","d"]) → Some(Action::DeleteLine)
  Count is 1 → no wrap needed
  Result: Complete(DeleteLine)

User types '5', then 'd', then 'd':
  After "5": Buffer: ["5"] → WaitForMore
  After "d": Buffer: ["5", "d"] → WaitForMore
  After "d": Buffer: ["5", "d", "d"]
    Parse: action_keys=["d","d"], count=5
    Lookup: get_action(["d","d"]) → Some(Action::DeleteLine)
    Wrap: Action::Count(5, DeleteLine)
    Result: Complete(Count(5, DeleteLine))
```

## 7. External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| `std::collections::BTreeMap` | Trie children storage | Deterministic iteration order |
| `std::collections::HashMap` | Action cache | Performance optimization |
| None | No external crates | Pure Rust implementation |

## 8. Error Handling

| Scenario | Handling |
|----------|----------|
| Invalid key sequence | Clear buffer, return `InvalidSequence` |
| Count overflow (>9999) | Cap at 9999, log warning |
| Empty action keys | Treat as invalid sequence |
| "0" followed by action | "0" is MoveToLineStart, not count |

### Overflow Handling

```rust
const MAX_COUNT: usize = 9999;

fn multiply_with_cap(a: usize, b: usize) -> usize {
    let result = a.saturating_mul(b);
    if result > MAX_COUNT {
        log::warn!("Count overflow: {} capped to {}", result, MAX_COUNT);
        MAX_COUNT
    } else {
        result
    }
}
```

## 9. Security

| Concern | Approach |
|---------|----------|
| Input validation | All keys are canonicalized before parsing |
| Buffer bounds | Max buffer size to prevent DoS |
| Count bounds | Cap at MAX_COUNT to prevent overflow |

## 10. Configuration

No new configuration required. The keymap is built statically when the mode is created.

## 11. Component Interactions

```
┌──────────────┐     ┌────────────────┐     ┌─────────────┐
│ NormalMode   │────▶│ CountParser    │────▶│ TrieKeymap  │
│ handle_key() │     │ parse()        │     │ get_action()│
└──────────────┘     └────────────────┘     └─────────────┘
       │                     │                      │
       │ 1. Add key to       │                      │
       │    buffer           │                      │
       │                     │                      │
       │ 2. Parse buffer     │                      │
       │    for counts       │                      │
       │                     │                      │
       │ 3. Extract action   │                      │
       │    keys             │                      │
       │                     │                      │
       │ 4. Lookup action    │                      │
       │    in trie          │                      │
       │                     │                      │
       │ 5. Wrap with count  │                      │
       │    and return       │                      │
       ▼                     ▼                      ▼
```

## 12. Platform Considerations

None - this is pure Rust with no platform-specific code.

## 13. Trade-offs

### Decision: BTreeMap vs HashMap for Trie Children

**Choice**: BTreeMap

**Reasoning**:
- Deterministic iteration order useful for debugging
- Better for prefix operations (need ordered iteration)
- Good enough performance for small branching factor (~10 keys)

**Impact**:
- Slightly more memory than HashMap
- Slightly slower for large branching factors

### Decision: Separate CountParser from TrieKeymap

**Choice**: Count parsing in separate component

**Reasoning**:
- Clear separation of concerns
- Easier to test count parsing independently
- Allows trie to remain focused on key sequence matching

**Impact**:
- Slightly more code to coordinate
- Enables future count modifier changes without touching trie

### Decision: Multiplicative vs Additive Sub-counts

**Choice**: Multiplicative

**Reasoning**:
- Matches user expectation ("2d2d" should be 4, not 3)
- Vim-compatible behavior
- Simpler mental model

**Impact**:
- Need to document the behavior clearly
- Some users might expect additive (but vim users expect multiplicative)

## 14. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Performance regression | Low | Medium | Benchmark before/after; trie should be faster |
| Breaking existing behavior | Low | High | Comprehensive test coverage |
| Count overflow | Low | Medium | Cap at MAX_COUNT with warning |
| Memory bloat with many bindings | Low | Low | Only store keys actually used; ~50 bindings is small |

## 15. Testing Strategy

### Unit Tests for CountParser

```rust
#[test]
fn test_single_digit_count() {
    let (keys, count) = CountParser::parse(&["5", "j"]);
    assert_eq!(keys, vec!["j"]);
    assert_eq!(count, 5);
}

#[test]
fn test_multi_digit_count() {
    let (keys, count) = CountParser::parse(&["5", "5", "d", "d"]);
    assert_eq!(keys, vec!["d", "d"]);
    assert_eq!(count, 55);
}

#[test]
fn test_sub_count() {
    let (keys, count) = CountParser::parse(&["d", "5", "d"]);
    assert_eq!(keys, vec!["d", "d"]);
    assert_eq!(count, 5);
}

#[test]
fn test_multiplicative_sub_counts() {
    let (keys, count) = CountParser::parse(&["2", "d", "2", "d"]);
    assert_eq!(keys, vec!["d", "d"]);
    assert_eq!(count, 4);
}

#[test]
fn test_zero_not_count() {
    let (keys, count) = CountParser::parse(&["0"]);
    assert_eq!(keys, vec!["0"]);
    assert_eq!(count, 1);
}

#[test]
fn test_mixed_multi_digit() {
    let (keys, count) = CountParser::parse(&["1", "2", "d", "3", "4", "d"]);
    assert_eq!(keys, vec!["d", "d"]);
    assert_eq!(count, 408);
}
```

### Unit Tests for TrieKeymap

```rust
#[test]
fn test_insert_and_get() {
    let mut keymap = TrieKeymap::new();
    keymap.insert("j".to_string(), Action::MoveDown);
    
    assert_eq!(keymap.get_action(&["j"]), Some(Action::MoveDown));
}

#[test]
fn test_sequence_insert() {
    let mut keymap = TrieKeymap::new();
    keymap.insert_sequence(vec!["g".to_string(), "g".to_string()], Action::MoveToFirstLine);
    
    assert_eq!(keymap.get_action(&["g", "g"]), Some(Action::MoveToFirstLine));
    assert_eq!(keymap.get_action(&["g"]), None);
}

#[test]
fn test_is_prefix() {
    let mut keymap = TrieKeymap::new();
    keymap.insert_sequence(vec!["g".to_string(), "g".to_string()], Action::MoveToFirstLine);
    
    assert!(keymap.is_prefix(&["g"]));
    assert!(keymap.is_prefix(&["g", "g"]));
    assert!(!keymap.is_prefix(&["g", "g", "g"]));
}
```

### Integration Tests

- Full key sequence with counts
- Multi-key sequences with counts
- Edge cases (empty, invalid sequences)
