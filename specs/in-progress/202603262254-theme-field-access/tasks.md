# Theme Field Access Simplification - Implementation Tasks

## Overview

Refactor the resolved theme API so rendering code accesses UI and syntax styles directly through `Theme.ui` and `Theme.syntax`, then remove the accessor methods and update all affected call sites and tests.

## Backend

- [x] **1.** Update the `Theme` data model and constructor to use direct style fields.
  - [x] **1.1** Rename the stored style collections on `Theme` to `ui` and `syntax`.
  - [x] **1.2** Update `Theme::new` to accept and store the renamed fields.
  - [x] **1.3** Remove the `ui_style()` and `syntax_style()` accessor methods from `Theme`.
  - [x] **1.4** Add or refresh documentation comments for the public `Theme` API affected by the field rename.

- [x] **2.** Update theme loading and any internal theme construction paths.
  - [x] **2.1** Adjust `Theme::new` call sites in the theme loader to pass the renamed fields. (depends on: 1)
  - [x] **2.2** Update any tests or helpers that construct `Theme` directly to use the new constructor signature. (depends on: 1)

- [x] **3.** Migrate rendering and theme-consumer code to direct field access.
  - [x] **3.1** Replace `theme.ui_style(...)` lookups with direct `theme.ui` field reads and concrete style field access. (depends on: 1)
  - [x] **3.2** Replace `theme.syntax_style(...)` lookups with direct `theme.syntax` field reads and concrete style field access. (depends on: 1)
  - [x] **3.3** Update all affected tests to assert against the direct fields instead of removed accessors. (depends on: 3)

## Testing

- [x] **4.** Verify the refactor with the project check/test workflow.
  - [x] **4.1** Run the relevant Rust tests for the theme, window, status bar, and tab group modules. (test: confirm direct field access compiles and behavior is unchanged)
  - [x] **4.2** Run `cargo check` to catch compile errors and warnings across the workspace. (test: ensure no stale accessor references remain)

## Completion Summary

| Item | Status |
| --- | --- |
| Backend | Complete |
| Testing | Complete |
| Total | 4 / 4 top-level tasks complete |
