# Unnamed Register Implementation Plan

## Overview

Implement the unnamed register (`""`) to match neovim behavior. The unnamed register acts as a "last operation" register — every yank, delete, and change writes to it, and paste reads from it by default when no explicit register is specified.

## Neovim Behavior Summary

- **Write path**: Every yank/delete/change writes to both the target register AND `""`.
- **Read path**: `p`/`P` without a register prefix reads from `""`.
- **Explicit register**: `"ap` reads from `a` directly, bypassing `""`. `"ayy` writes to `a` AND `""`.

## Current urvim Behavior

- **Write path**: Yank → configured yank register (`y`). Delete → `d`. Change → `c`.
- **Read path**: `p`/`P` always reads from the configured yank register.
- **No unnamed register exists.**

## Changes Required

### 1. Add unnamed register constant (`register.rs`)

Add a constant to `RegisterName`:

```rust
impl RegisterName {
    pub const UNNAMED: Self = Self('"');
}
```

No changes to `from_prefix()` — the `"` character is not a valid prefix selector (it triggers the register prefix state machine), so this won't conflict.

### 2. Modify write path (`commands.rs`)

In `store_register_content()`, after writing to the target register, also write the same content to the unnamed register:

```rust
pub(super) fn store_register_content(
    &self,
    explicit: Option<RegisterName>,
    role: DefaultRegisterRole,
    text: String,
    kind: RegisterContentKind,
) {
    let register = Self::resolved_register_name(explicit, role);
    let content = RegisterContent::new(text, kind);
    globals::with_register_store_mut(|store| {
        store.set(register, content.clone());
        store.set(RegisterName::UNNAMED, content);
    });
}
```

This means every yank, delete, and change operation updates `""` in addition to the target register.

### 3. Modify read path (`commands.rs`)

Change `paste_register_content()` to read from the unnamed register when no explicit register is given, instead of falling back to the yank role:

```rust
pub(super) fn paste_register_content(
    &mut self,
    explicit: Option<RegisterName>,
    after: bool,
) -> ActionResult {
    let register = explicit.unwrap_or(RegisterName::UNNAMED);
    let Some(content) = globals::with_register_store(|store| store.get(register)) else {
        return ActionResult::Handled;
    };
    // ... rest unchanged
}
```

Remove the `role` parameter since paste no longer needs it — unnamed register is always the default.

### 4. Update call sites (`widget.rs`)

Update the `PasteAfter` and `PasteBefore` arms to drop the role argument:

```rust
Some(ActionKind::PasteAfter) => {
    self.paste_register_content(action.register, true)
}
Some(ActionKind::PasteBefore) => {
    self.paste_register_content(action.register, false)
}
```

### 5. Update documentation (`docs/registers.md`)

- Add unnamed register section explaining it mirrors every operation
- Update "Paste Behavior" section: `p`/`P` reads from `""` by default
- Update "Differences From Vim" to remove the unnamed register bullet
- Update Quick Reference table

## Files to Modify

| File | Change |
|------|--------|
| `crates/urvim_core/src/register.rs` | Add `RegisterName::UNNAMED` constant |
| `crates/urvim_core/src/window/commands.rs` | Modify `store_register_content` to also write to `""`. Modify `paste_register_content` to read from `""` when no explicit register. Remove `role` param from `paste_register_content`. |
| `crates/urvim_core/src/window/widget.rs` | Update `PasteAfter`/`PasteBefore` call sites |
| `crates/urvim_core/src/window/tests.rs` | Update 10 existing paste tests that pre-populate `RegisterName('y')` to use `RegisterName::UNNAMED` instead. Add 10 new unnamed register tests. |
| `docs/registers.md` | Update documentation |

## Existing Tests to Update

The following 10 tests in `crates/urvim_core/src/window/tests.rs` pre-populate `RegisterName('y')` and then call `Action::paste_after()` / `Action::paste_before()` without an explicit register. After the change, paste reads from `RegisterName::UNNAMED` by default, so these tests will break unless updated to pre-populate `RegisterName::UNNAMED` instead:

| Line | Test | Change |
|------|------|--------|
| 6398 | `paste_after_characterwise_puts_cursor_at_end_of_pasted_text` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6419 | `paste_before_characterwise_puts_cursor_at_start_of_pasted_text` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6440 | `paste_after_characterwise_multiline_puts_cursor_at_end` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6461 | `paste_after_linewise_puts_cursor_at_end_of_last_pasted_line` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6482 | `paste_before_linewise_puts_cursor_at_start_of_first_pasted_line` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6503 | `paste_after_linewise_multiline_puts_cursor_at_end_of_last_line` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6524 | `paste_before_linewise_multiline_puts_cursor_at_start_of_first_line` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6545 | `paste_after_characterwise_inserts_at_cursor_not_after_character` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6566 | `paste_after_linewise_multiple_lines_inserts_all_content` | `RegisterName('y')` → `RegisterName::UNNAMED` |
| 6593 | `paste_before_linewise_multiple_lines_inserts_all_content` | `RegisterName('y')` → `RegisterName::UNNAMED` |

The `test_paste_after_uses_explicit_named_register` test at line 6376 does NOT need updating because it uses `.with_register(RegisterName('z'))`, which bypasses the unnamed register.

The `test_yank_line_populates_yank_register_and_paste_after_uses_it` test at line 6302 does NOT need updating because `YankLine` will write to both `'y'` and `""`, and `paste_after()` will read from `""`.

## Behavioral Matrix

| Operation | Target Register | Also Writes to `""` | Reads from |
|-----------|----------------|---------------------|------------|
| `yy` | configured yank (default `y`) | yes | — |
| `dd` | configured delete (default `d`) | yes | — |
| `cc` | configured change (default `c`) | yes | — |
| `"ayy` | `a` | yes | — |
| `p` | — | no | `""` |
| `"ap` | — | no | `a` |

## New Tests

Add these tests to `crates/urvim_core/src/window/tests.rs`, following the existing register test patterns (Pattern A: dispatch action then read store, Pattern B: pre-populate store then paste and assert buffer).

### Write Path: unnamed register mirrors every operation

```rust
#[test]
fn test_yank_line_writes_to_unnamed_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("hello\nworld");
    let mut window = Window::new(buffer);

    window.dispatch_action(&Action::new(ActionKind::YankLine));

    let content = globals::with_register_store(|store| store.get(RegisterName::UNNAMED))
        .expect("unnamed register should have content");
    assert_eq!(content.text, "hello");
    assert_eq!(content.kind, RegisterContentKind::Linewise);
}

#[test]
fn test_delete_line_writes_to_unnamed_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("hello\nworld");
    let mut window = Window::new(buffer);

    window.dispatch_action(&Action::new(ActionKind::DeleteLine));

    let content = globals::with_register_store(|store| store.get(RegisterName::UNNAMED))
        .expect("unnamed register should have content");
    assert_eq!(content.text, "hello");
    assert_eq!(content.kind, RegisterContentKind::Linewise);
}

#[test]
fn test_yank_selection_writes_to_unnamed_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    // Simulate visual selection yank
    let action = Action::new(ActionKind::YankSelection)
        .with_from_mode(ModeKind::Visual)
        .with_to_mode(ModeKind::Normal);
    window.dispatch_action(&action);

    let content = globals::with_register_store(|store| store.get(RegisterName::UNNAMED))
        .expect("unnamed register should have content");
    assert_eq!(content.kind, RegisterContentKind::Characterwise);
}
```

### Write Path: explicit named register also writes to unnamed

```rust
#[test]
fn test_explicit_named_register_also_writes_to_unnamed() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("hello\nworld");
    let mut window = Window::new(buffer);

    let action = Action::new(ActionKind::YankLine)
        .with_register(RegisterName('a'));
    window.dispatch_action(&action);

    // Both register 'a' and unnamed should have the content
    let named = globals::with_register_store(|store| store.get(RegisterName('a')))
        .expect("register 'a' should have content");
    let unnamed = globals::with_register_store(|store| store.get(RegisterName::UNNAMED))
        .expect("unnamed register should have content");

    assert_eq!(named.text, "hello");
    assert_eq!(unnamed.text, "hello");
    assert_eq!(named.kind, RegisterContentKind::Linewise);
    assert_eq!(unnamed.kind, RegisterContentKind::Linewise);
}
```

### Read Path: paste defaults to unnamed register

```rust
#[test]
fn test_paste_after_reads_from_unnamed_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    // Pre-populate unnamed register, but NOT the yank register
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName::UNNAMED,
            RegisterContent::new("pasted".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    window.dispatch_action(&Action::paste_after());

    let text = window.buffer_view().with_buffer(|b| b.to_string());
    assert_eq!(text, "apastedb");
}

#[test]
fn test_paste_before_reads_from_unnamed_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName::UNNAMED,
            RegisterContent::new("pasted".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    window.dispatch_action(&Action::paste_before());

    let text = window.buffer_view().with_buffer(|b| b.to_string());
    assert_eq!(text, "pastedab");
}
```

### Read Path: explicit register bypasses unnamed

```rust
#[test]
fn test_paste_with_explicit_register_bypasses_unnamed() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    globals::with_register_store_mut(|store| {
        store.set(
            RegisterName::UNNAMED,
            RegisterContent::new("from-unnamed".to_string(), RegisterContentKind::Characterwise),
        );
        store.set(
            RegisterName('z'),
            RegisterContent::new("from-z".to_string(), RegisterContentKind::Characterwise),
        );
    });

    let buffer = Buffer::from_str("ab");
    let mut window = Window::new(buffer);

    window.dispatch_action(&Action::paste_after().with_register(RegisterName('z')));

    let text = window.buffer_view().with_buffer(|b| b.to_string());
    assert_eq!(text, "afrom-zb");
}
```

### Integration: yank then paste via unnamed register

```rust
#[test]
fn test_yank_then_paste_uses_unnamed_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("hello\nworld");
    let mut window = Window::new(buffer);

    // Yank line (writes to yank register AND unnamed register)
    window.dispatch_action(&Action::new(ActionKind::YankLine));

    // Move cursor down, paste without explicit register
    window.buffer_view().set_cursor(Cursor::new(1, 0));
    window.dispatch_action(&Action::paste_after());

    let text = window.buffer_view().with_buffer(|b| b.to_string());
    assert_eq!(text, "hello\nhello\nworld");
}
```

### Integration: delete then paste via unnamed register

```rust
#[test]
fn test_delete_then_paste_uses_unnamed_register() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("hello\nworld");
    let mut window = Window::new(buffer);

    // Delete line (writes to delete register AND unnamed register)
    window.dispatch_action(&Action::new(ActionKind::DeleteLine));

    // Paste without explicit register -- should paste the deleted line
    window.dispatch_action(&Action::paste_before());

    let text = window.buffer_view().with_buffer(|b| b.to_string());
    assert_eq!(text, "hello\nworld");
}
```

### Overwrite: unnamed register reflects last operation

```rust
#[test]
fn test_unnamed_register_overwritten_by_subsequent_operation() {
    let _register_guard = globals::set_test_register_store(RegisterStore::new());
    let buffer = Buffer::from_str("first\nsecond\nthird");
    let mut window = Window::new(buffer);

    // Yank first line
    window.dispatch_action(&Action::new(ActionKind::YankLine));

    // Delete second line (should overwrite unnamed register)
    window.buffer_view().set_cursor(Cursor::new(1, 0));
    window.dispatch_action(&Action::new(ActionKind::DeleteLine));

    // Paste should get the deleted line, not the yanked line
    window.dispatch_action(&Action::paste_before());

    let text = window.buffer_view().with_buffer(|b| b.to_string());
    assert_eq!(text, "second\nfirst\nthird");
}
```

## Verification

1. `cargo check` — no warnings
2. `cargo test` — all existing register tests pass + new tests pass
3. Manual verification:
   - `yy` then `p` pastes yanked text (via unnamed register)
   - `dd` then `p` pastes deleted text (via unnamed register)
   - `"ayy` then `p` pastes yanked text (unnamed register was updated)
   - `"ayy` then `"ap` pastes from register `a`
   - `dd` then `"ayy` then `p` pastes the yanked text (unnamed register was updated by yank)
