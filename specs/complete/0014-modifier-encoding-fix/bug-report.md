# [Bug]: Modifier Encoding Edge Case with Invalid Values

## Summary
The `Modifiers::from_kitty_encoding()` function in `keys.rs` does not properly validate input values. When given invalid modifier values (like 4 which is not a valid Kitty protocol value), it produces incorrect modifier results due to the use of `saturating_sub(1)`.

## Severity: Low

- Impact: May cause incorrect modifier interpretation for malformed escape sequences
- Frequency: Rare - only occurs with non-compliant terminals sending invalid modifier values
- Workaround: Use terminals that follow Kitty protocol correctly

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest |
| OS | Any |
| Terminal | Any |

## Reproduction Steps

1. Terminal sends CSI-u sequence with modifier value 4 (e.g., `\x1b[27;4u` for Left+???)
2. Application calls `Modifiers::from_kitty_encoding(4)`
3. Observe: Returns `Modifiers(3)` = ALT | CTRL combined

## Expected Behavior

Modifier value 4 should either:
- Return a sensible default (like no modifiers)
- Return an error/unrecognized modifier state
- The function should validate against known valid values (0, 2, 3, 5, 6, 7, 8, 9, 10, etc.)

## Actual Behavior

`from_kitty_encoding(4)` returns `Modifiers(3)` which means ALT | CTRL, which is incorrect since modifier value 4 alone is not valid in the Kitty protocol.

## Root Cause

The function uses `value.saturating_sub(1)` which converts:
- 4 - 1 = 3 (0b11 = ALT | CTRL)

But valid Kitty modifier values are:
- 0 = no modifiers
- 2 = Shift (1+1)
- 3 = Alt (1+2)
- 5 = Ctrl (1+4)
- 6 = Ctrl+Shift (1+4+1)
- 7 = Alt+Ctrl (1+2+4)
- 8 = Alt+Ctrl+Shift (1+2+4+1)
- And higher for Super, Hyper, Meta with modifiers

Value 4 alone (just 1+3 which doesn't correspond to any valid combination) is invalid.

Location: `src/terminal/keys.rs:89-94`

## Solution Approach

**Chosen**: Add validation to only accept known valid modifier values

**Reasoning**:
- Prevents incorrect modifier interpretation
- More robust against non-compliant terminals
- Clear behavior for invalid input

**Rejected alternatives**:
- Keep current behavior: Incorrect results for invalid inputs
- Use wrapping_sub: Could produce different incorrect results

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/terminal/keys.rs` | Modify | Add validation in `from_kitty_encoding()` |

## Edge Cases

- Test with value 1 (also invalid - should map to no modifiers or error)
- Test with very large values (128+, for num_lock modifier)
- Verify valid values still work correctly
