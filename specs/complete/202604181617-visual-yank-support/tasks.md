# Visual Yank Support - Implementation Tasks

## Overview

Implement yank support for character-wise visual mode and visual-line mode, reuse the existing register and paste model, and add regression coverage plus docs updates.

## Backend

- [x] **1.** Add yank handling to visual mode and visual-line mode
  - [x] **1.1** Bind `y` in the character-wise visual mode handler and the visual-line mode handler
  - [x] **1.2** Route visual yank through the existing register-aware selection capture path
  - [x] **1.3** Exit to normal mode after a successful visual yank
  - [x] **1.4** Preserve the active text kind when writing the register payload

- [x] **2.** Keep visual yank aligned with existing selection semantics
  - [x] **2.1** Ensure character-wise visual yank captures the exact selected span
  - [x] **2.2** Ensure visual-line yank captures whole-line boundaries
  - [x] **2.3** Leave the buffer unchanged when the yank succeeds
  - [x] **2.4** Treat unresolved or empty selection states as no-ops that do not overwrite registers

## Testing

- [x] **3.** Add regression coverage for visual yank behavior
  - [x] **3.1** Add tests for `y` in character-wise visual mode covering buffer unchanged, register write, and return to normal mode
  - [x] **3.2** Add tests for `y` in visual-line mode covering whole-line capture, register write, and return to normal mode
  - [x] **3.3** Add tests that confirm characterwise visual yanks paste inline and linewise visual yanks paste as whole lines
  - [x] **3.4** Add tests that verify an explicit register prefix still works with visual yank
  - [x] **3.5** Add tests that verify empty or unresolved visual yank states do not overwrite existing register contents

## Docs

- [x] **4.** Update user-facing docs and validate the build
  - [x] **4.1** Update [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) to document `y` in visual and visual-line mode
  - [x] **4.2** Update [`specs/glossary.md`](/Users/ryan/Dev/urvim/specs/glossary.md) if register terminology needs a dedicated entry
  - [x] **4.3** Run `cargo check` and the targeted visual/register test suites, then fix any regressions or clippy warnings

## Completion Summary

| Item | Status | Notes |
| --- | --- | --- |
| 1. Visual yank binding | Complete | Added `y` handling to both visual modes |
| 2. Selection semantics | Complete | Preserved selection kind and no-op behavior |
| 3. Regression coverage | Complete | Covered visual yank, register write, and paste shape |
| 4. Docs and verification | Complete | Updated motions docs, glossary, and ran checks |
