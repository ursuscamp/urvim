# Key String Trie Inserts - Technical Design

## Architecture Overview
This feature adds a small canonical key-string parser next to `TrieKeymap` and exposes a convenience insertion method that accepts a single string. The parser uses the same canonical token shape already produced by `Key::canonical_string()`, so the helper can be used for both one-key and multi-key bindings without introducing a second notation.

The existing vector-based insertion path remains available. The new helper is a readability layer over the same trie insertion behavior, not a new keymap format.

## Interface Design

| Interface | Input | Output | Description |
|-----------|-------|--------|-------------|
| `TrieKeymap::insert_str(&mut self, keys: &str, action: Action)` | Canonical key string | `()` | Parses a canonical string and inserts the equivalent trie binding. |
| `TrieKeymap::insert_sequence(&mut self, keys: Vec<String>, action: Action)` | Parsed key sequence | `()` | Existing sequence-based insertion path, retained for dynamic callers. |

### Parsing Contract
- Non-bracketed characters are treated as individual key tokens.
- Bracketed canonical names are treated as a single token.
- The parser must not split a valid bracketed token into pieces.
- The parser must not invent bindings from malformed syntax.

## Data Models

No new persistent data model is required. The parser produces the same `Vec<String>` sequence shape that the trie already stores internally.

### Parsed Token Sequence

| Field | Type | Meaning |
|-------|------|---------|
| tokens | `Vec<String>` | Ordered key tokens extracted from a canonical string |

## Key Components

### Canonical Key String Parser

**Responsibilities:**
- Convert a canonical key string into ordered trie tokens.
- Recognize bracketed canonical key names as atomic tokens.
- Keep the parser logic local to keymap registration so it stays easy to reason about.

**Public API:**
- None required if the parser remains private to the keymap module.

**Behavior:**
- `"gg"` becomes `["g", "g"]`
- `"diw"` becomes `["d", "i", "w"]`
- `"<C-s>"` becomes `["<C-s>"]`
- `"d<LessThan>"` becomes `["d", "<LessThan>"]`

### TrieKeymap String Insertion

**Responsibilities:**
- Provide a readable insertion path for literal bindings.
- Reuse the existing trie node creation and action storage logic.
- Preserve the current behavior of `insert` and `insert_sequence`.

**Public API:**
- `insert_str(&mut self, keys: &str, action: Action)`
- `insert_sequence(&mut self, keys: Vec<String>, action: Action)`
- `insert(&mut self, key: String, action: Action)`

### Call-Site Migration

**Responsibilities:**
- Replace manual `vec![...].to_string()` construction for literal editor bindings.
- Keep dynamic or programmatically assembled sequences on `insert_sequence` where that remains clearer.
- Update tests and helper code so the new method becomes the default for literal trie bindings.

## User Interaction

### Before
```rust
trie_keymap.insert_sequence(vec!["g".to_string(), "g".to_string()], Action::MoveToFirstLine);
trie_keymap.insert("<C-s>".to_string(), Action::SaveBuffer(None));
```

### After
```rust
trie_keymap.insert_str("gg", Action::MoveToFirstLine);
trie_keymap.insert_str("<C-s>", Action::SaveBuffer(None));
```

## External Dependencies
None. This feature stays within the existing editor and terminal code.

## Error Handling
The parser should treat malformed canonical strings as invalid input rather than silently producing an incorrect key sequence. The helper must never degrade a malformed string into a different valid binding.

## Security
No security-sensitive behavior is introduced. The parser only processes internal key binding literals.

## Configuration
No configuration changes are required.

## Component Interactions

```text
Literal canonical string
    -> key-string parser
    -> Vec<String> token sequence
    -> existing trie insertion logic
    -> keymap lookup at runtime
```

## Platform Considerations
The parser must remain Unicode-safe because canonical strings can include Unicode character keys. Bracketed special tokens stay ASCII and unambiguous across platforms.
