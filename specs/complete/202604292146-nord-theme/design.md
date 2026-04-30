# Nord Theme - Technical Design

## Architecture Overview

This change adds a new built-in theme asset named `Nord` and registers it in the existing theme loading pipeline. The implementation stays inside the current theme system: the theme is authored as TOML, embedded at compile time, parsed during startup, and resolved into the runtime `Theme` model alongside the existing built-ins.

The theme should be as faithful as practical to `shaunsingh/nord.nvim`, but it must fit urvim's theme schema. That means we map Nord's upstream intent onto urvim's existing UI surface names and syntax tags rather than introducing Nord-only runtime options.

### Flow

```text
Startup
  -> load embedded built-in theme TOML files
  -> parse each file into a resolved Theme
  -> register themes by exact name
  -> select requested theme from config or CLI

Rendering
  -> renderers request the active Theme by name/tag
  -> theme returns resolved Style overlays
  -> renderer combines them with the current cell/base style
```

## Implementation Strategy

### 1. Add the built-in Nord theme asset

Create `src/theme/builtin/nord.toml` with:

- Nord palette values from the upstream color set
- a dark default style using the Nord background/foreground
- UI styles for the editor surfaces urvim already supports:
  - status bar
  - modified marker
  - selection
  - active line
  - tabs
  - gutter
  - window body and border lines
  - prompts, picker location, and notifications
- syntax styles aligned with urvim's current theme vocabulary:
  - comment, constant, function, namespace, keyword, markup, number, operator, punctuation, string, type, variable

The file should use the same schema conventions as the other built-in TOML themes, so no loader changes are needed beyond registration.

### 2. Register Nord as a built-in theme

Extend the built-in theme source list in `src/theme/model.rs` so `ThemeRegistry::load_builtin()` includes Nord.

Behavior:

- the registry should expose `Nord` by exact name
- existing default theme behavior remains unchanged
- unknown theme selection still fails with the current startup error path

### 3. Update user-facing documentation

Update the configuration documentation to mention Nord in the built-in theme list or example theme names.

If there are other docs or comments that enumerate built-in themes, keep them in sync so Nord is discoverable.

### 4. Add regression tests

Add tests that prove:

- the registry loads Nord
- Nord is selectable by name
- key Nord surfaces resolve to the intended styles
- the default theme remains unchanged

Prefer tests that exercise the actual registry and theme resolver rather than duplicating raw TOML assertions.

## Data Model Notes

Nord does not require changes to the core theme schema. It should remain a plain built-in TOML theme with the current palette/default/highlight model.

Upstream `nord.nvim` includes plugin-specific and runtime-flag behavior such as transparency and plugin highlight groups. Those are out of scope unless urvim already has a direct equivalent surface. The theme should focus on the UI and syntax names the editor actually uses.

## Validation and Compatibility

- The registry must still reject duplicate theme names.
- Startup theme selection must continue to use exact matching.
- Existing themes must not change names, kinds, or defaults.
- The theme should remain compatible with the current UI rendering path without new configuration flags.

## Verification Plan

- add unit tests for registry membership and representative style resolution
- run `cargo fmt`
- run `cargo check`
- run the targeted test suite for theme loading and any affected UI rendering tests
