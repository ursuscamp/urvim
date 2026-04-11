# Filetype Glyph Metadata - Implementation Tasks

## Overview

Add optional filetype glyph metadata to syntax definitions, add an editor config option for advanced glyph capabilities, and update the tab bar and status bar to render icons when `nerdfont` is enabled. Keep the text-only presentation unchanged when glyph metadata is absent or the capability is disabled.

## Backend

- [ ] **1.** Extend syntax metadata to carry optional glyph fields. (depends on: none)
  - [ ] **1.1** Add optional glyph and glyph color fields to the raw and compiled syntax metadata structures.
  - [ ] **1.2** Add a raw glyph color type and parse it into the terminal color type used by renderers.
  - [ ] **1.3** Validate glyph strings and glyph colors in the syntax loader.
  - [ ] **1.4** Update built-in syntax TOML files with glyphs and default colors for languages that should show icons.
  - [ ] **1.5** Add regression tests for loading syntax metadata with glyph fields, missing fields, and invalid glyph colors.

- [ ] **2.** Add editor configuration support for advanced glyph capabilities. (depends on: none)
  - [ ] **2.1** Add an `AdvancedGlyphCapability` enum with `nerdfont` as the initial value.
  - [ ] **2.2** Add an optional `advanced_glyphs` field to the config file schema and the resolved config.
  - [ ] **2.3** Parse, validate, and store the enabled advanced glyph capabilities as a set-like collection.
  - [ ] **2.4** Add config-loading tests for missing, valid, duplicate, and unknown advanced glyph values.

- [ ] **3.** Expose syntax glyph presentation to the UI renderers. (depends on: 1, 2)
  - [ ] **3.1** Add a small syntax presentation helper or accessor that returns the display label plus optional glyph metadata for the active buffer syntax.
  - [ ] **3.2** Update tab rendering so an enabled glyph appears before the file name and contributes to width calculations and clipping.
  - [ ] **3.3** Update status bar rendering so an enabled glyph appears before the syntax label and existing modified-marker positioning remains correct.
  - [ ] **3.4** Keep the existing text-only output unchanged when glyphs are unavailable or disabled.

## Testing

- [ ] **4.** Add UI and integration-style regression coverage for glyph rendering. (depends on: 1, 2, 3)
  - [ ] **4.1** Add tests for tab bar rendering with glyphs enabled, disabled, and omitted.
  - [ ] **4.2** Add tests for status bar rendering with glyphs enabled, disabled, and omitted.
  - [ ] **4.3** Add tests that verify glyph colors are applied without changing surrounding label styles.
  - [ ] **4.4** Run `cargo check` and the relevant test targets for config, syntax, tab group, status bar, and layout code.

## Documentation

- [ ] **5.** Update user-facing docs and grammar references. (depends on: 1, 2, 3)
  - [ ] **5.1** Document the new `advanced_glyphs` config field in `docs/config.md`.
  - [ ] **5.2** Document the new glyph and glyph color syntax metadata fields in `docs/syntax/grammar.md`.
  - [ ] **5.3** Update any built-in syntax examples or references that describe syntax metadata layout.

## Completion Summary

| Item | Status | Notes |
| --- | --- | --- |
| 1. Syntax metadata support | Pending | Optional glyph and glyph color fields, plus builtin metadata updates |
| 2. Advanced glyph config | Pending | `nerdfont` capability gate in startup config |
| 3. UI glyph rendering | Pending | Tab bar and status bar icon display |
| 4. Regression tests | Pending | Loader, config, and renderer coverage |
| 5. Documentation | Pending | Config and syntax grammar docs |
