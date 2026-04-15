# Active Line Highlighting - Implementation Tasks
## Overview
Implement optional active-line highlighting for the focused window in normal mode. The change spans startup config, theme schema/resolution, window rendering, built-in themes, and user-facing documentation.

## Backend
- [x] **1.** Add the `active_line` config toggle to the startup config model and TOML schema.  
  - [x] **1.1** Extend `Config` and `PartialConfig` with the new boolean field.  
  - [x] **1.2** Resolve the field with a default of `false` in config loading.  
  - [x] **1.3** Add config tests covering default resolution and TOML override behavior.  
- [x] **2.** Extend the theme schema and resolved theme model with a dedicated active-line UI style.  
  - [x] **2.1** Add `active_line` to the raw UI schema, UI key enum, and resolved `UiStyles`.  
  - [x] **2.2** Update theme loading so all built-in and custom themes can resolve the new style.  
  - [x] **2.3** Add model and loader tests for the new UI style field and schema coverage.  

## Rendering
- [x] **3.** Apply the active-line style during window rendering when the feature is enabled.  
  - [x] **3.1** Thread the config value and focused-window state into the render decision.  
  - [x] **3.2** Highlight exactly the cursor line in the focused window while in normal mode only.  
  - [x] **3.3** Preserve syntax highlighting, selection highlighting, and existing fallback behavior when the feature is disabled.  
  - [x] **3.4** Add render tests that verify focused-window, mode, and disabled-state behavior.  

## Theme Assets
- [x] **4.** Update the built-in themes to define an active-line UI style.  
  - [x] **4.1** Choose slightly lighter background values for each built-in theme's active-line style.  
  - [x] **4.2** Keep the style subtle and consistent with each theme's existing palette.  

## Documentation
- [x] **5.** Document the new config option and theme UI style.  
  - [x] **5.1** Update `docs/config.md` with the new `active_line` setting.  
  - [x] **5.2** Update any theme documentation or inline docs that describe the closed UI style set.  
  - [x] **5.3** Ensure public docs/comments added during implementation follow the repository's doc-comment standards.  

## Testing
- [x] **6.** Run the relevant test suite and fix any regressions introduced by the change.  
  - [x] **6.1** Add or update unit tests in config, theme, and window modules.  
  - [x] **6.2** Run `cargo test` for the affected modules and `cargo check` for the workspace.  
  - [x] **6.3** Address any clippy warnings surfaced by the touched code paths.  

## Completion Summary
| Item | Status |
| --- | --- |
| Backend config toggle | Done |
| Theme schema/model update | Done |
| Rendering behavior | Done |
| Built-in theme updates | Done |
| Documentation updates | Done |
| Test and lint pass | Done |
