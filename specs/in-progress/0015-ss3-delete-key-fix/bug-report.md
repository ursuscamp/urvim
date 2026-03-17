# [Bug]: SS3 Delete Key Mapping May Be Non-Standard

## Summary
The escape sequence parser maps SS3 `Oq` to `KeyCode::Delete` in `try_parse_ss3()`, but this mapping is not defined in the Kitty keyboard protocol specification. This could cause incorrect key detection from non-compliant terminals.

## Severity: Low

- Impact: May cause incorrect Delete key detection from some terminals
- Frequency: Uncommon - only affects terminals that send non-standard SS3 sequences
- Workaround: Use Kitty protocol (CSI-u) or standard CSI-tilde sequences

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest |
| OS | Any |
| Terminal | Non-standard terminals sending SS3 Delete |

## Reproduction Steps

1. Terminal sends SS3 sequence `\x1bOq` for Delete key
2. Application parses it as `KeyCode::Delete`
3. This behavior may be incorrect per Kitty spec

## Expected Behavior

According to Kitty protocol, SS3 codes are:
- OP, OQ, OR, OS: F1-F4
- OH, OF: Home, End  
- OV, OW: PageUp, PageDown

There is no standard SS3 code for Insert or Delete.

## Actual Behavior

The current code maps `Oq` to `KeyCode::Delete`:
```rust
// escape.rs line 402
b'q' => KeyCode::Delete,
```

## Root Cause

The SS3 mapping table includes Insert (`Op`) and Delete (`Oq`) which are not part of the Kitty protocol standard. These were likely added based on xterm behavior but may cause issues.

Location: `src/terminal/escape.rs:387-407`

## Solution Approach

**Chosen**: Remove the non-standard SS3 Insert/Delete mappings

**Reasoning**:
- Align with Kitty protocol specification
- Standard CSI-tilde sequences (for Insert and Delete are still handled via `\x1b[2~` and `\x1b[3~`)
- Maintains compatibility with terminals using standard Kitty/legacy sequences

**Rejected alternatives**:
- Keep mapping: May cause incorrect behavior with some terminal/keyboard combinations

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/terminal/escape.rs` | Modify | Remove `b'p'` and `b'q'` from SS3 match |

## Edge Cases

- Verify CSI-tilde Delete (`\x1b[3~`) still works
- Test with terminals that only send SS3 Delete
- Consider if any common terminal relies on this non-standard mapping
