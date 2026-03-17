# Comprehensive Keyboard Test Coverage - Technical Design

## Architecture Overview

This is a testing-only feature. No production code changes required. We will add test cases to `src/terminal/mod.rs` following existing test patterns.

## Test Structure

Existing tests in `mod.rs` follow this pattern:

```rust
#[test]
fn test_kitty_csi_u_xxx() {
    let mut terminal = create_terminal(b"\x1b[CODE;MODIFIERSu".to_vec());
    let event = terminal.read_event().unwrap();
    assert_eq!(event, Event::Key(Key::with_modifiers(KeyCode::XXX, Modifiers::YYY)));
}
```

## Test Cases to Add

### CSI-u Key Tests

| Sequence | Key | Modifiers | Description |
|----------|-----|-----------|-------------|
| `\x1b[2u` | Tab | none | CSI-u code 2 = Tab |
| `\x1b[4u` | Enter | none | CSI-u code 4 = Enter |
| `\x1b[5u` | Home | none | CSI-u code 5 = Home |
| `\x1b[6u` | End | none | CSI-u code 6 = End |
| `\x1b[7u` | PageUp | none | CSI-u code 7 = PageUp |
| `\x1b[8u` | PageDown | none | CSI-u code 8 = PageDown |
| `\x1b[10u` | Insert | none | CSI-u code 10 = Insert |
| `\x1b[24u` | Up | none | CSI-u code 24 = Up |
| `\x1b[25u` | Down | none | CSI-u code 25 = Down |
| `\x1b[26u` | Right | none | CSI-u code 26 = Right |
| `\x1b[27u` | Left | none | CSI-u code 27 = Left |
| `\x1b[127u` | Backspace | none | CSI-u code 127 = Backspace |

### CSI-u Modifier Tests

| Sequence | Key | Modifiers | Description |
|----------|-----|-----------|-------------|
| `\x1b[2;2u` | Tab | SHIFT | Shift+Tab |
| `\x1b[4;2u` | Enter | SHIFT | Shift+Enter |
| `\x1b[10;2u` | Insert | SHIFT | Shift+Insert |
| `\x1b[127;2u` | Backspace | SHIFT | Shift+Backspace |

### Legacy CSI Tilde Tests

| Sequence | Key | Description |
|----------|-----|-------------|
| `\x1b[1~` | Home | CSI 1~ = Home (alternate) |
| `\x1b[7~` | Home | CSI 7~ = Home (alternate) |
| `\x1b[8~` | End | CSI 8~ = End (alternate) |

## Test File Location

All tests should be added to `src/terminal/mod.rs` in the existing `#[cfg(test)]` module, near the existing CSI-u tests (around line 1290).

## Error Handling

No error handling needed - tests verify correct behavior.

## Security

No security concerns - tests only.
