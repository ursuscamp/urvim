# Character Scan Range Motions - Implementation Tasks
## Overview

Add operator-pending range support for the existing `f`, `F`, `t`, and `T` character scan family, verify the behavior with focused regression tests, and update the motions documentation so the new operator workflows are discoverable.

## Backend

- [x] **1.** Extend the operator target model so character scan motions can resolve as motion ranges.
  - [x] **1.1** Add a character-scan target type that stores the scan kind, direction, and target character.
  - [x] **1.2** Teach operator application to resolve the new target type using the existing character scan search logic.
  - [x] **1.3** Preserve the current normal-mode behavior for `f`, `F`, `t`, and `T`.

- [x] **2.** Wire operator-pending key handling for the character scan family.
  - [x] **2.1** Route `f`, `F`, `t`, and `T` through the operator-pending path when an operator is active.
  - [x] **2.2** Keep count parsing and count multiplication aligned with the existing motion rules.
  - [x] **2.3** Ensure failed target resolution cancels the pending operator cleanly.

- [x] **3.** Preserve repeat-search state when character scan motions are used as operator targets.
  - [x] **3.1** Update the stored last-search state from the underlying character scan resolution.
  - [x] **3.2** Verify that `;` and `,` continue to repeat the latest search after operator-pending range use.

## Testing

- [x] **4.** Add regression tests for the new operator-pending range behavior.
  - [x] **4.1** Cover `ct:` and `cf:`-style edits on a single line.
  - [x] **4.2** Cover forward and backward delete/change cases for `f/F/t/T`.
  - [x] **4.3** Cover count-prefixed range motions and the multiplicative count path.
  - [x] **4.4** Cover the no-match case to ensure the buffer is unchanged.
  - [x] **4.5** Cover repeat-search state after a range motion.

- [x] **5.** Run the project checks relevant to the feature.
  - [x] **5.1** Run `cargo test` for the motion and operator coverage.
  - [x] **5.2** Run `cargo check` to catch compile errors and warnings.

## Docs

- [x] **6.** Update motion documentation to describe the new range behavior.
  - [x] **6.1** Document the operator-pending character scan workflow in `docs/motions.md`.
  - [x] **6.2** Add examples for `ct:`, `dfx`, `dtx`, `dFx`, and `dTx`.
  - [x] **6.3** Update `specs/glossary.md` only if implementation terminology changes during the work.

## Completion Summary

| Item | Status |
|------|--------|
| Backend | Done |
| Testing | Done |
| Docs | Done |
| Overall | Done |
