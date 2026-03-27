# Unified Config Framework - Implementation Tasks

## Overview

Total: 7 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Add the config module and canonical startup schema
  - [x] **1.1** Create `src/config.rs` and export the public config types and load error type
  - [x] **1.2** Define the resolved `Config` type and the partial TOML-backed input type with `theme` as the first field
  - [x] **1.3** Add documentation comments for the new public module, types, and methods

- [x] **2.** Implement TOML config parsing and XDG config-file discovery
  - [x] **2.1** Add filesystem and XDG config location handling for `$XDG_CONFIG_HOME/urvim/config.toml` and `$XDG_CONFIG_DIRS/urvim/config.toml`
  - [x] **2.2** Load the first config file found in XDG search order, or return the default configuration source when no file exists
  - [x] **2.3** Parse the config file as TOML and reject unknown fields or invalid schema values
  - [x] **2.4** Surface clear errors for unreadable files, parse failures, and validation failures

- [x] **3.** Add config merging and global storage
  - [x] **3.1** Merge file values with CLI overrides so CLI theme values take precedence
  - [x] **3.2** Apply the existing default theme when neither source provides a theme
  - [x] **3.3** Add global config storage and read access in `src/globals.rs`
  - [x] **3.4** Keep the active theme storage separate while sourcing the theme name from the resolved config

- [x] **4.** Wire unified config loading into startup
  - [x] **4.1** Update `src/main.rs` to load and resolve config before entering the main editor loop
  - [x] **4.2** Store the resolved config in globals before selecting the active theme
  - [x] **4.3** Resolve the active theme from the config theme name and keep existing startup failure behavior for unknown themes
  - [x] **4.4** Ensure startup continues cleanly when no config file is present

- [x] **5.** Keep config documentation synchronized
  - [x] **5.1** Maintain `docs/config.md` as the user-facing summary of the config file location, precedence rules, and schema
  - [x] **5.2** Update `docs/config.md` whenever the canonical config schema or precedence behavior changes

- [x] **6.** Add unit tests for config loading and precedence
  - [x] **6.1** Test that missing config files fall back to defaults
  - [x] **6.2** Test that TOML config files are loaded from the XDG search path
  - [x] **6.3** Test that CLI theme overrides win over config file theme values
  - [x] **6.4** Test that invalid TOML or invalid schema data fails startup cleanly
  - [x] **6.5** Test that resolved config is stored in globals and can be read back

- [x] **7.** Verify and fix regressions
  - [x] **7.1** Run `cargo check` and fix any compile errors or warnings
  - [x] **7.2** Run the targeted test suite for config loading, globals, and startup integration
  - [x] **7.3** Run the full test suite before marking the work complete

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 5 | 5 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **7** | **7** | **100%** |
