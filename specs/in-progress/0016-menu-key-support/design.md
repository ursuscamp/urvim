# Menu Key Support - Technical Design

## Architecture Overview

This feature adds support for the Menu key (also known as Application key or context menu key) to the keyboard handling system. It involves:
1. Adding a new `Menu` variant to the `KeyCode` enum
2. Adding escape sequence parsing for CSI 29~
3. Adding canonical string representation
4. Adding unit tests

## Interface Design

### KeyCode Enum Addition

```rust
// In keys.rs - KeyCode enum
pub enum KeyCode {
    // ... existing variants ...
    /// Menu/Application/Context menu key
    Menu,
}
```

### Parser Interface

| Input | Output | Description |
|-------|--------|-------------|
| `\x1b[29~` | `KeyCode::Menu` | CSI-tilde Menu key |
| `\x1b[29;2~` | `Key::with_modifiers(Code::Menu, SHIFT)` | Shift+Menu |

## Data Models

### KeyCode Enum Change

| Field | Type | Description |
|-------|------|-------------|
| Menu | KeyCode variant | Menu/Application/Context menu key |

## Key Components

### 1. KeyCode Enum (keys.rs)

Add `Menu` variant to the enum and implement `special_name()` method:

```rust
impl KeyCode {
    pub fn special_name(&self) -> Option<&'static str> {
        match self {
            // ... existing cases ...
            KeyCode::Menu => Some("Menu"),
        }
    }
}
```

### 2. Escape Sequence Parser (escape.rs)

Add CSI 29~ handling in `try_parse_csi_tilde()`:

```rust
let key = match num {
    // ... existing cases ...
    29 => KeyCode::Menu,  // Menu key (CSI 29~)
    // Note: code 16 is also not handled (F6 duplicate in some terminals)
    _ => return None,
};
```

## User Interaction

The Menu key will be parsed as:
- Plain Menu: `<Menu>`
- Shift+Menu: `<S-Menu>`
- Ctrl+Menu: `<C-Menu>`
- Alt+Menu: `<A-Menu>`

## Error Handling

- If CSI 29~ with unknown modifiers: parse with available modifiers (standard behavior)
- If terminal doesn't support CSI 29~: falls through to escape key (acceptable legacy behavior)

## Security

No security concerns - this is input parsing only.

## Configuration

No configuration needed - keyboard protocol is determined by terminal capabilities.

## Trade-offs

**Decision**: Add Menu key support

**Reasoning**:
- Low implementation complexity
- Improves user experience for keyboards with Menu key
- Aligns with Kitty protocol specification

**Impact**: 
- Minimal code change
- No breaking changes to existing functionality

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Terminal doesn't send CSI 29~ | Low | Low | Falls back to Escape, acceptable |
