# Unified Config Framework

## Summary

Create a single startup configuration framework that reads persistent settings from a TOML config file in the XDG config directories, merges those settings with command-line flags, and stores the resolved result globally for the rest of the editor startup and rendering flow. The initial config surface centers on theme selection, with theme as the first canonical config value in the TOML file schema.

## Problem Statement

urvim currently handles startup behavior through a mix of direct CLI parsing and ad hoc globals, which makes it hard to add persistent configuration and easy to duplicate the same setting across multiple code paths. Theme selection is especially split between CLI startup logic and runtime state. A unified config framework would give the editor one resolved source of truth, make persistent user preferences possible, and create a clean path for future settings.

## User Stories

- As a user, I want to set my preferred theme in a config file, so that urvim starts with my chosen appearance without requiring a CLI flag every time.
- As a user, I want command-line flags to override my saved config, so that I can make temporary startup changes without editing files.
- As a maintainer, I want one resolved config object stored globally, so that startup and rendering code can read the same settings consistently.
- As a contributor, I want config loading to use the standard XDG config locations, so that urvim follows familiar desktop conventions.

## Functional Requirements

- [ ] **REQ-001**: The editor shall resolve startup settings into a single unified configuration object before entering the main editor loop.
- [ ] **REQ-002**: The unified configuration object shall be stored in global state so that startup and rendering code can access the resolved settings without re-parsing CLI arguments or config files.
- [ ] **REQ-003**: The editor shall load its TOML config file from the user XDG config locations using the urvim config path.
- [ ] **REQ-004**: If multiple config files exist in the XDG search path, the editor shall use the first one found in XDG order.
- [ ] **REQ-005**: The config file shall be optional; if no config file exists, startup shall continue using default settings.
- [ ] **REQ-006**: The config file shall define theme as the first canonical config value in the TOML file schema.
- [ ] **REQ-007**: The CLI shall expose a theme setting that feeds into the same unified configuration object as the config file.
- [ ] **REQ-008**: When both the config file and CLI provide a theme value, the CLI value shall take precedence.
- [ ] **REQ-009**: When neither the config file nor the CLI provides a theme value, the editor shall continue to use the existing default theme.
- [ ] **REQ-010**: The resolved configuration object shall expose the active theme to the rest of the application through global access.
- [ ] **REQ-011**: If the TOML config file exists but cannot be parsed or validated, the editor shall report a configuration error and refuse to continue startup.
- [ ] **REQ-012**: If the config file is present but unreadable due to an I/O error, the editor shall report the error and refuse to continue startup.
- [ ] **REQ-013**: The unified config framework shall be structured so additional config values can be added later without changing the precedence model between file and CLI sources.

## Non-Functional Requirements

- **Consistency**: All startup code paths shall observe the same resolved config values once initialization completes.
- **Predictability**: Config file values shall be loaded deterministically from the XDG search path and overridden by CLI values in a documented order.
- **Maintainability**: The configuration framework should concentrate source-of-truth logic in a small number of startup and globals modules.
- **Extensibility**: The initial design should support adding more config fields beyond theme without reworking the merge model.

## Acceptance Criteria

- [ ] **AC-001**: Starting urvim with no config file and no theme flag still selects the existing default theme.
- [ ] **AC-002**: A theme value in the XDG config file becomes the active theme when no CLI override is present.
- [ ] **AC-003**: A CLI theme flag overrides the theme value loaded from the config file.
- [ ] **AC-004**: The editor reads the TOML config file from the XDG config path rather than from the current working directory.
- [ ] **AC-005**: Startup fails with a clear error when the config file exists but contains invalid configuration data.
- [ ] **AC-006**: After startup, application code can read the resolved config from global state instead of carrying CLI arguments through the call stack.
- [ ] **AC-007**: The canonical TOML config file schema presents theme as the first config value.

## Out of Scope

- Runtime config reloading after startup
- A config UI or editor-integrated settings screen
- Non-XDG config locations
- Multiple named config profiles or per-project config stacking
- Adding new user-facing settings beyond the unified framework foundation

## Assumptions

- The initial config surface is centered on theme selection because that is the only user-facing startup setting currently needed.
- The config file can be treated as a local user preference file and does not need secret handling or remote synchronization.
- The editor already has a global state mechanism that can be extended to store resolved startup config.
- Theme selection should continue to work even when no config file exists, preserving current startup behavior.

## Dependencies

- Existing CLI argument parsing
- Existing theme selection and loading logic
- Existing global state helpers
- An implementation path for locating and reading files from XDG config directories
