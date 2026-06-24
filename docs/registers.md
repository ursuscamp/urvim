# urvim Registers

This document describes urvim's register model and how it differs from Vim.

urvim keeps registers session-wide, so copied text survives mode changes, pane focus changes, and repeated paste operations while the editor is open.

## What urvim Supports

urvim uses a smaller register model than Vim:

- an unnamed register that mirrors every operation
- three built-in default registers for yank, delete, and change
- explicit named registers selected with a prefix
- characterwise and linewise register content

## Unnamed Register

The unnamed register (`""`) is the default register. It automatically mirrors every yank, delete, and change operation.

When you paste with `p` or `P` without specifying a register prefix, the unnamed register is the source. This means:

- `yy` then `p` pastes the yanked text
- `dd` then `p` pastes the deleted text
- `"ayy` then `p` still pastes the yanked text (the unnamed register is updated by every operation)

When you specify an explicit register for a write operation (e.g., `"ayy`), the text is written to both the named register and the unnamed register.

## Default Registers

urvim keeps separate implicit registers for the three main editing operators:

- yank writes to the yank register (and the unnamed register)
- delete writes to the delete register (and the unnamed register)
- change writes to the change register (and the unnamed register)

These default registers are always available and do not depend on explicit register selection.

The default registers are also directly targetable with selector keys:

- `y` selects the yank register
- `d` selects the delete register
- `c` selects the change register

Their configured destinations can be changed with the `default_registers` config option.

## Named Registers

Named registers are selected with a register prefix:

- press `"`
- press a single lowercase ASCII letter
- run the next command using that register

Lowercase letters other than `y`, `d`, and `c` are available as user-named registers.

Examples:

- `"ayw` yanks into register `a`
- `"adw` deletes into register `a`
- `"ap` pastes from register `a`
- In visual mode, `"` then `a` then `y` yanks the current selection into register `a`
- In visual-line mode, `V` then `y` yanks the selected lines into the default yank register

Only the next command uses the selected register. After that command runs, the prefix state is cleared.

If `default_registers` remaps an operator destination, the matching selector key still chooses that operator's configured default destination.

## Paste Behavior

`p` and `P` read from the unnamed register unless an explicit register prefix is present.

- characterwise content is pasted inline
- linewise content is pasted as whole lines

That means a linewise yank behaves like a line insertion, while a characterwise yank behaves like an in-line insertion.

## Differences From Vim

urvim intentionally does not implement Vim's full register system.

- no numbered register history
- no clipboard register behavior
- no black-hole register
- no macro registers in this first version

Instead, urvim keeps the model focused on the registers needed for the editor's yank, delete, change, and paste flow.

## Error Handling

- An invalid register prefix cancels the current command sequence.
- A paste with no stored register value is a no-op.
- Empty resolved targets do not overwrite register contents.

## Quick Reference

| Command | Effect |
|---------|--------|
| `y` | Yank into the yank register and the unnamed register |
| `d` | Delete into the delete register and the unnamed register |
| `c` | Change into the change register and the unnamed register |
| `p` | Paste after the cursor or current line from the unnamed register |
| `P` | Paste before the cursor or current line from the unnamed register |
| `v` then `y` | Yank a character-wise visual selection |
| `V` then `y` | Yank a linewise visual selection |
| `"` + `y` / `d` / `c` | Target the yank, delete, or change register directly |
| `"` + other lowercase letter | Target a user-named register |
