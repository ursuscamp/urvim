# Canonical Key String Representation - Technical Design

## Architecture Overview

This feature adds a method to the existing `Key` type in `src/terminal/keys.rs` that converts any keypress to a canonical string representation. The implementation is a pure transformation with no side effects, fitting seamlessly into the existing key handling architecture.

```
┌─────────────────┐    canonical_string()    ┌─────────────────┐
│      Key       │ ──────────────────────→  │     String      │
│  (KeyCode +    │                         │  (canonical     │
│   Modifiers)   │                         │   representation)│
└─────────────────┘                         └─────────────────┘
```

## Interface Design

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `Key::canonical_string(&self)` | `&Key` | `String` | Returns canonical string representation |

### Method Signature

```rust
impl Key {
    /// Returns a canonical string representation of this key.
    ///
    /// The canonical representation follows these rules:
    /// - Printable characters (including emojis) are returned as-is
    /// - Three characters are exceptions: space → `<Space>`, < → `<LessThan>`, > → `<GreaterThan>`
    /// - Special keys use angle bracket notation: `<Enter>`, `<Up>`, `<F1>`, etc.
    /// - Modifier combinations use the format `<M-key>` where M is the modifier prefix
    /// - Modifier order is: Ctrl → Alt → Shift → Super → Hyper → Meta
    /// - Shift + letter is normalized to uppercase
    pub fn canonical_string(&self) -> String;
}
```

## Data Models

No new data models are required. The implementation uses the existing:
- `KeyCode` enum (already defined)
- `Modifiers` struct (already defined)
- `Key` struct (already defined)

### KeyCode to Special Name Mapping

| KeyCode | Special Name |
|---------|--------------|
| `Enter` | "Enter" |
| `Backspace` | "Backspace" |
| `Tab` | "Tab" |
| `Esc` | "Esc" |
| `Delete` | "Delete" |
| `Insert` | "Insert" |
| `Up` | "Up" |
| `Down` | "Down" |
| `Left` | "Left" |
| `Right` | "Right" |
| `Home` | "Home" |
| `End` | "End" |
| `PageUp` | "PageUp" |
| `PageDown` | "PageDown" |
| `F1` - `F12` | "F1" - "F12" |
| `Null` | "Null" |

### Modifier Prefix Mapping

| Modifier | Prefix |
|----------|--------|
| CTRL | "C" |
| ALT | "A" |
| SHIFT | "S" |
| SUPER | "Su" |
| HYPER | "H" |
| META | "M" |

## Key Components

### 1. Canonical String Module

**Location**: `src/terminal/keys.rs` (addition to existing module)

**Responsibilities:**
- Convert `KeyCode` to special name string
- Convert `Modifiers` to prefix string in canonical order
- Handle special cases for space, less-than, greater-than
- Handle Shift normalization for letters

**Public API:**
- `Key::canonical_string(&self) -> String`

**Internal Helper Functions:**
- `keycode_to_special_name(code: &KeyCode) -> Option<&str>` - Converts KeyCode to special name if not a Char
- `modifiers_to_prefixes(modifiers: Modifiers) -> Vec<&str>` - Returns modifier prefixes in canonical order
- `is_shiftable_letter(c: char) -> bool` - Checks if character is a letter a-z or A-Z
- `needs_special_representation(c: char) -> bool` - Checks if character is space, <, or >

## User Interaction

### Direct Usage

```rust
use urvim::terminal::keys::{Key, KeyCode, Modifiers};

// Basic character
let key = Key::new(KeyCode::Char('a'));
assert_eq!(key.canonical_string(), "a");

// Ctrl+a
let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL);
assert_eq!(key.canonical_string(), "<C-a>");

// Shift+a → normalized to uppercase
let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::SHIFT);
assert_eq!(key.canonical_string(), "A");

// Ctrl+Alt+a
let key = Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL | Modifiers::ALT);
assert_eq!(key.canonical_string(), "<A-C-a>");

// Space character (special case)
let key = Key::new(KeyCode::Char(' '));
assert_eq!(key.canonical_string(), "<Space>");

// Arrow key
let key = Key::new(KeyCode::Up);
assert_eq!(key.canonical_string(), "<Up>");
```

## External Dependencies

None. This is a pure internal implementation using existing types.

## Error Handling

The `canonical_string` method always succeeds and returns a valid String. There are no error conditions.

## Security

Not applicable - this is a pure transformation function with no security implications.

## Configuration

No configuration needed - the canonical representation is hardcoded and deterministic.

## Component Interactions

```
User Code
    │
    ▼
Key::canonical_string()
    │
    ├──▶ keycode_to_special_name()  ──→ special name string or None
    │
    ├──▶ modifiers_to_prefixes()    ──→ modifier prefixes in order
    │
    └──▶ format output              ──→ final String
```

## Platform Considerations

The implementation is platform-independent. It operates on the abstract `Key` type which is already platform-agnostic.

## Trade-offs

**Decision**: Use short modifier prefixes (C, A, S, Su, H, M) instead of full names

**Reasoning**:
- Keeps canonical strings shorter and more readable
- Matches common conventions in vim-like editors
- Reduces chance of user confusion

**Impact**:
- Users need to learn the prefix abbreviations
- Ambiguity between Super and Shift could theoretically occur (but they're separate in practice)

**Alternative Considered**: Full modifier names (Ctrl, Alt, Shift, Super, Hyper, Meta)
- Rejected because strings become too long (e.g., "<Ctrl-Alt-a>" vs "<A-C-a>")

## Algorithm Details

### Shift Normalization Logic

1. If modifiers contain SHIFT and the character is a letter (a-z or A-Z):
   - Return the uppercase version of the letter
2. If modifiers contain SHIFT and the character has a shifted representation on US keyboard:
   - Return the shifted character (e.g., '1' → '!', '[' → '{')
3. If modifiers contain SHIFT and the key is a special key:
   - Include Shift in the modifier prefix (e.g., `<S-Enter>`)
4. Otherwise:
   - Include Shift in the modifier prefix

### Modifier Order Algorithm

The canonical order is: **Ctrl → Alt → Shift → Super → Hyper → Meta**

Implementation:
```rust
fn modifiers_to_prefixes(modifiers: Modifiers) -> Vec<&'static str> {
    let mut prefixes = Vec::new();
    if modifiers.has_ctrl() { prefixes.push("C"); }
    if modifiers.has_alt() { prefixes.push("A"); }
    if modifiers.has_shift() { prefixes.push("S"); }
    if modifiers.has_super() { prefixes.push("Su"); }
    if modifiers.has_hyper() { prefixes.push("H"); }
    if modifiers.has_meta() { prefixes.push("M"); }
    prefixes
}
```

### Special Character Handling

```rust
fn needs_special_representation(c: char) -> bool {
    matches!(c, ' ' | '<' | '>')
}

fn special_name_for_char(c: char) -> Option<&'static str> {
    match c {
        ' ' => Some("Space"),
        '<' => Some("LessThan"),
        '>' => Some("GreaterThan"),
        _ => None,
    }
}
```

## Testing Strategy

### Unit Tests Required

1. **Character tests**: a-z, A-Z, 0-9, punctuation, emojis
2. **Special key tests**: All KeyCode variants that are not Char
3. **Modifier tests**: Each modifier individually and in combination
4. **Shift normalization tests**: Letters, numbers/punctuation, special keys
5. **Special character tests**: space, less-than, greater-than
6. **Modifier order tests**: Verify canonical ordering

### Test Examples

```rust
#[test]
fn test_basic_characters() {
    assert_eq!(Key::new(KeyCode::Char('a')).canonical_string(), "a");
    assert_eq!(Key::new(KeyCode::Char('Z')).canonical_string(), "Z");
    assert_eq!(Key::new(KeyCode::Char('1')).canonical_string(), "1");
}

#[test]
fn test_special_exceptions() {
    assert_eq!(Key::new(KeyCode::Char(' ')).canonical_string(), "<Space>");
    assert_eq!(Key::new(KeyCode::Char('<')).canonical_string(), "<LessThan>");
    assert_eq!(Key::new(KeyCode::Char('>')).canonical_string(), "<GreaterThan>");
}

#[test]
fn test_modifier_order() {
    // Ctrl before Alt
    assert_eq!(
        Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL | Modifiers::ALT).canonical_string(),
        "<A-C-a>"
    );
    // Ctrl, Alt before Shift
    assert_eq!(
        Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT).canonical_string(),
        "<A-C-A>"
    );
}

#[test]
fn test_shift_normalization() {
    // Shift + letter = uppercase
    assert_eq!(
        Key::with_modifiers(KeyCode::Char('a'), Modifiers::SHIFT).canonical_string(),
        "A"
    );
    // Shift + number = shifted character
    assert_eq!(
        Key::with_modifiers(KeyCode::Char('1'), Modifiers::SHIFT).canonical_string(),
        "!"
    );
}
```

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Incomplete KeyCode coverage | Low | Medium | Add comprehensive tests for all KeyCode variants |
| Unicode handling edge cases | Low | Low | Test with various Unicode characters including emojis |
| Modifier order confusion | Low | Low | Document clearly and test combinations thoroughly |
