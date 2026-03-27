# Theme System - Implementation Tasks

## Overview

Total: 7 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Backend

- [x] **1.** Add the theme module foundation and closed style-key schema
  - [x] **1.1** Create `src/theme/mod.rs` and export the public theme types, registry API, and load error type
  - [x] **1.2** Add typed UI and syntax key enums for the predefined schema keys used by the editor
  - [x] **1.3** Define the raw TOML models for `name`, `palette`, `default`, `ui`, and `syntax` with closed section structs
  - [x] **1.4** Add documentation comments for the new public module, types, and methods

- [x] **2.** Implement TOML parsing, validation, and style resolution
  - [x] **2.1** Add `serde` and `toml` dependencies to `Cargo.toml`
  - [x] **2.2** Implement parsing of embedded TOML theme sources into raw theme models
  - [x] **2.3** Validate required sections, reject unknown `ui` or `syntax` keys, and validate palette references
  - [x] **2.4** Resolve palette colors into `terminal::Color` and classify themes as ANSI or true color
  - [x] **2.5** Convert partial `default`, `ui`, and `syntax` styles into resolved `terminal::Style` values layered from the default style
  - [x] **2.6** Add startup errors for duplicate theme names, invalid palette values, and invalid style references

- [x] **3.** Add the built-in theme registry and embedded TOML sources
  - [x] **3.1** Create the built-in theme loader and registry in `src/theme/registry.rs`
  - [x] **3.2** Add statically included TOML files for `Friday Night`, `Saturday Morning`, `rose-pine`, `dracula`, `tokyo-night`, and `catppuccin`
  - [x] **3.3** Ensure the default built-in theme is `Friday Night`
  - [x] **3.4** Keep the built-in TOML files aligned with the closed UI and syntax schema keys

- [x] **4.** Wire theme selection into startup and CLI handling
  - [x] **4.1** Extend `src/main.rs` to accept a `--theme` flag
  - [x] **4.2** Load the built-in registry during startup before the editor enters its main loop
  - [x] **4.3** Select the requested theme by exact name, or fall back to `Friday Night` when the flag is omitted
  - [x] **4.4** Fail startup with a clear error when the requested theme does not exist
  - [x] **4.5** Pass the active theme into the editor root so renderers can read it

- [x] **5.** Update UI renderers to use themed styles instead of hard-coded colors
  - [x] **5.1** Replace hard-coded tab bar styling in `src/tab_group.rs` with predefined theme UI keys
  - [x] **5.2** Replace hard-coded status bar styling in `src/status_bar.rs` with predefined theme UI keys
  - [x] **5.3** Update window and gutter rendering paths to consume theme-provided styles where applicable
  - [x] **5.4** Keep `Screen` and terminal output unchanged so they continue to accept final `terminal::Style` values

## Testing

- [x] **6.** Add unit tests for theme parsing, validation, and resolution
  - [x] **6.1** Test that valid built-in themes parse successfully and classify correctly
  - [x] **6.2** Test that unknown `ui` or `syntax` keys fail validation
  - [x] **6.3** Test that palette references resolve through the default style and inherit unspecified fields
  - [x] **6.4** Test that invalid palette values and missing palette references fail startup validation
  - [x] **6.5** Test that the default theme is `Friday Night` when no CLI theme is provided

- [x] **7.** Run verification and fix regressions
  - [x] **7.1** Run `cargo check` and fix any compile errors or warnings
  - [x] **7.2** Run the targeted test suite for the new theme module and affected UI renderers
  - [x] **7.3** Run the full test suite before marking the work complete

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Backend | 5 | 5 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **7** | **7** | **100%** |
