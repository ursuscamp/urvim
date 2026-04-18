# Basic Register Support - Implementation Tasks

## Overview

Implement a small editor-wide register system with separate default yank/delete/change registers, yank commands that mirror existing delete/change targets, and paste commands that read from the yank register by default while still allowing an explicit register prefix.

## Backend

- [x] **1.** Add the register data model and global store
  - [x] **1.1** Introduce register types for the three default registers plus explicit named registers
  - [x] **1.2** Add a session-wide register store to the global state layer
  - [x] **1.3** Store both the copied text and whether it is characterwise or linewise

- [x] **2.** Add register-aware editing helpers
  - [x] **2.1** Add a focused buffer helper or equivalent extraction path that can capture the text for a resolved operator target
  - [x] **2.2** Extend the operator flow so yank can reuse the same target resolution as delete and change
  - [x] **2.3** Add paste actions that can insert register contents inline or as whole lines

- [x] **3.** Teach normal mode to parse register prefixes
  - [x] **3.1** Add a small register-selection state that activates after `"`
  - [x] **3.2** Route the next command through the selected register when a prefix is present
  - [x] **3.3** Register `y`, `p`, and `P` in the normal-mode keymap
  - [x] **3.4** Keep count parsing working with and without an explicit register prefix

- [x] **4.** Wire register behavior into window command handling
  - [x] **4.1** Make yank copy text into the selected register without mutating the buffer
  - [x] **4.2** Make delete and change continue mutating the buffer while also writing to the selected default or explicit register
  - [x] **4.3** Make `p` and `P` paste from the resolved register and honor characterwise versus linewise placement
  - [x] **4.4** Ensure empty or invalid register operations are no-ops and do not overwrite registers

## Testing

- [x] **5.** Add regression coverage for register behavior
  - [x] **5.1** Test that yank leaves the buffer unchanged and fills the yank register
  - [x] **5.2** Test that delete writes to the delete register without overwriting the yank register
  - [x] **5.3** Test that change writes to the change register and still enters insert mode
  - [x] **5.4** Test that explicit register selection redirects yank and paste
  - [x] **5.5** Test that linewise yanks paste as whole lines and characterwise yanks paste inline
  - [x] **5.6** Test that invalid register prefixes do not mutate the buffer

- [x] **6.** Update user-facing docs and validate the build
  - [x] **6.1** Update [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) with the new yank, register-prefix, and paste commands
  - [x] **6.2** Add or update any glossary or help text needed to describe the simplified register model
  - [x] **6.3** Run `cargo check` and fix any build or warning issues introduced by the change
  - [x] **6.4** Run the relevant test suite for editor actions, registers, and paste behavior

## Completion Summary

| Area | Status | Notes |
| --- | --- | --- |
| 1. Register model | Complete | Added default and named registers with content metadata |
| 2. Yank/paste helpers | Complete | Reused target resolution and added register-aware paste |
| 3. Normal mode parsing | Complete | Register-prefix state and bindings are in place |
| 4. Window behavior | Complete | Copy, delete, change, and paste wiring is implemented |
| 5. Tests | Complete | Register regressions and edge cases are covered |
| 6. Docs/build | Complete | Docs were updated and checks passed |
