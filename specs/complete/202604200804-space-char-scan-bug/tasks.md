# Space Char Scan Bug - Implementation Tasks

## Overview
Fix character scan motions by adding explicit space-target bindings in normal mode and operator-pending mode, and add regression coverage for the failing motions.

## Implementation
- [x] **1.** Add explicit space-target bindings for character scan motions.
  - [x] **1.1** Add normal-mode bindings for `f<Space>`, `F<Space>`, `t<Space>`, and `T<Space>`.
  - [x] **1.2** Route each explicit binding to the existing motion logic with a literal space target.

- [x] **2.** Add operator-pending bindings for space-target range motions.
  - [x] **2.1** Add the range-motion bindings that correspond to the explicit space-target motions.
  - [x] **2.2** Confirm repeated spaces behave like repeated matches for other characters.

- [x] **3.** Add regression tests for space-target character scans.
  - [x] **3.1** Test `f<Space>` on `hello world` moves to the space.
  - [x] **3.2** Test `t<Space>`, `F<Space>`, and `T<Space>` behave correctly.
  - [x] **3.3** Test at least one operator-pending case such as `d<Space>`.
  - [x] **3.4** Confirm tabs remain unchanged and only literal space is affected.

- [x] **4.** Run validation.
  - [x] **4.1** Run `cargo check`.
  - [x] **4.2** Run the relevant tests or the full test suite if needed.

## Completion Summary
| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Implementation | 2 | 2 | Done |
| Testing | 1 | 1 | Done |
| Validation | 1 | 1 | Done |
| Total | 4 | 4 | Done |
