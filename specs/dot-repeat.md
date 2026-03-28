# Dot Repeat Checklist

This is a memory aid for the Vim behaviors that `.` can repeat. The goal is to keep the eventual urvim implementation aligned with Vim's repeat model, not to define the implementation order.

## Core Rule

- [x] `.` repeats the last **change**.
- [x] A supplied count on `.` controls how many times the completed change is replayed.
- [x] The original change's count still applies to the structural edit inside each replay.
- [x] Repeating preserves the original change semantics as much as possible, including cursor-relative behavior.
- [ ] `.` does not repeat undo/redo.
- [ ] `.` does not repeat command-line `:` commands.
- [ ] `@:` is separate from `.` and repeats the last command-line command instead.

## Direct Normal-Mode Changes

### Operator-Pending Edits

- [x] `d{motion}`
- [x] `c{motion}`
- [ ] `y{motion}` only when `'cpoptions'` includes `y`
- [x] Text-object based edits such as `diw`, `daw`, `ciw`, `caw`
- [x] Linewise operator forms such as `dd`, `cc`, `D`, and `C`
- [x] Counted operator forms such as `3dw`, `d3w`, `3d2w`

### Single-Key Buffer Changes

- [x] `x` and `X`
- [ ] `s` and `S`
- [ ] `r`
- [ ] `R`
- [ ] `p` and `P`
- [ ] `gp` and `gP`
- [x] `J` and `gJ`
- [ ] `<<` and `>>`
- [ ] `~`
- [ ] `g~`
- [ ] `gu`
- [ ] `gU`
- [ ] `gq`
- [ ] `gw`
- [ ] `g?`
- [ ] `<`
- [ ] `>`
- [ ] `=`
- [ ] `!{motion}{filter-command}`
- [ ] Any other normal-mode command that changes buffer text

## Insert and Append Style Changes

- [x] `i`
- [x] `I`
- [x] `a`
- [x] `A`
- [x] `o`
- [x] `O`
- [ ] `gi`
- [ ] `gI`
- [ ] `R`
- [ ] `gR`

- [x] The full inserted text is repeated, not just the entry command.
- [x] Insert-mode edits that happen before leaving Insert mode are part of the repeated change.
- [x] Ending the insert or replace session with `<Esc>` is included in the recorded change.
- [x] The replay count from `.` is separate from the original change count used by the structural edit.

## Visual-Mode Changes

- [ ] Characterwise Visual changes are repeatable with `.`.
- [ ] Linewise Visual changes are repeatable with `.`.
- [ ] Blockwise Visual changes are repeatable with `.`.
- [ ] Repeating a Visual change uses the same selection size semantics as Vim.
- [ ] Repeating a Visual change re-applies the operator on the comparable area in the new location.

## Register and Count Details

- [ ] If the original change used a numbered register, the register number advances on repeat.
- [ ] Repeating a change preserves the original register choice when that matters.
- [x] Counts are stored as part of the change and can be overridden by a new count on `.`.

## Things `.` Does Not Cover

- [ ] Undo (`u`)
- [ ] Redo (`CTRL-R`)
- [ ] Command-line repeats via `@:` are not `.` repeats
- [ ] Pure motions with no buffer change
- [ ] Search repetition (`n`, `N`)
- [ ] Macro replay (`@{register}`, `@@`)

## Notes For Future Implementation

- [x] Treat `.` as replaying the last recorded change transaction.
- [x] Make sure the transaction captures both the edit target and the text payload inserted or replaced.
- [x] Keep the original edit count distinct from the `.` repeat count so inserted text is not duplicated.
- [x] Keep cursor positioning stable enough that repeated edits feel like Vim.
- [ ] Keep this checklist updated as new motion, text-object, or insert behaviors are added.

## Current Status

- [x] Dot-repeat now replays both the structural change and committed insert text for supported change-and-insert flows.
- [x] Insert payloads are captured at insert-mode exit and applied programmatically during repeat replay.
- [ ] Visual-mode repeat behavior remains out of scope for now.

## Sources

- [Vim repeat help](https://vimhelp.org/repeat.txt.html)
- [Vim user manual, repeating a change](https://vimhelp.org/usr_04.txt.html#04.3)
- [Vim insert mode commands](https://vimhelp.org/insert.txt.html#inserting)
- [Vim visual mode repeating](https://vimhelp.org/visual.txt.html#visual-repeat)
