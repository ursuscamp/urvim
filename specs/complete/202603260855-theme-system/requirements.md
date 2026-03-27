# Theme System

## Summary

Add a theme system for urvim that styles editor UI and future syntax elements through TOML theme files. Themes must be selectable from the command line, include a palette and a required default style, provide concrete named styles in closed `ui` and `syntax` sections, and ship with both ANSI and true color built-in themes that are parsed at startup from statically included TOML sources.

## Problem Statement

urvim currently lacks a first-class theming model that can consistently style editor surfaces and evolve into syntax highlighting. Without a theme system, colors and text attributes cannot be configured in a reusable way, the editor cannot switch appearance at startup, and future syntax highlighting would have no shared style vocabulary with the UI.

## User Stories

- As a user, I want to choose a theme at startup, so that I can use a preferred appearance without recompiling the editor.
- As a theme author, I want to define palette colors and concrete UI and syntax styles in TOML, so that themes are readable, reusable, and easy to maintain.
- As a renderer, I want every rendered style to inherit from the theme default style, so that shared styling such as background color does not need to be repeated everywhere.
- As a UI developer, I want named styles for specific UI and syntax elements, so that rendering code can request a single resolved style for each concrete element.

## Functional Requirements

- [ ] **REQ-001**: The editor shall load themes from TOML documents that define a theme name, a palette section, a default style section, a `ui` section, and a `syntax` section.
- [ ] **REQ-002**: The editor shall ship built-in themes as TOML files that are statically included in the binary and parsed during startup.
- [ ] **REQ-003**: The built-in theme set shall include two ANSI themes named `Friday Night` and `Saturday Morning`.
- [ ] **REQ-004**: The built-in theme set shall include true color themes for Rose Pine, Dracula, Tokyo Night, and Catppuccin.
- [ ] **REQ-005**: The editor shall use the built-in `Friday Night` ANSI theme as the default theme when no `--theme` flag is provided.
- [ ] **REQ-006**: Every theme shall define a default style that is applied to each rendered cell before any UI or syntax-specific style is used.
- [ ] **REQ-007**: The command-line interface shall accept a `--theme` flag that selects the active theme by name before the editor starts rendering.
- [ ] **REQ-008**: If the `--theme` flag names a theme that is not available, the editor shall report the configuration error and refuse to continue startup.
- [ ] **REQ-009**: Each theme palette entry shall map a palette color name to a color value.
- [ ] **REQ-010**: The `ui` section shall contain only the predefined UI keys supported by urvim, with exactly one style for each key.
- [ ] **REQ-011**: The `syntax` section shall contain only the predefined syntax keys supported by urvim, with exactly one style for each key.
- [ ] **REQ-012**: The editor shall reject any theme that contains an unknown key in the `ui` or `syntax` sections.
- [ ] **REQ-013**: UI styles and syntax styles shall both support partial style definitions that may omit any field not being overridden.
- [ ] **REQ-014**: Every color reference in `default`, `ui`, and `syntax` styles shall refer to a named color from the palette.
- [ ] **REQ-015**: The theme system shall resolve all palette color references during startup.
- [ ] **REQ-016**: A style definition shall be invalid if it directly uses a literal color value instead of a palette color name.
- [ ] **REQ-017**: The editor shall reject a theme during startup if any style references a palette color name that is not defined in that theme.
- [ ] **REQ-018**: The final style for a named UI or syntax element shall be produced by layering that element's partial style on top of the theme default style.
- [ ] **REQ-019**: Style resolution shall not support arbitrary stacking of multiple named styles for a single rendered element.
- [ ] **REQ-020**: The theme system shall classify a theme as a 256-color theme when every palette color value is an ANSI color value.
- [ ] **REQ-021**: The theme system shall classify a theme as a true color theme when any palette color value is a true color value.
- [ ] **REQ-022**: UI-facing styling APIs shall expose predefined UI keys and predefined syntax keys instead of caller-provided strings.

## Non-Functional Requirements

- Compatibility: The theme format and resolution rules shall support both 256-color terminals and true color terminals without defining separate styling systems.
- Reliability: Theme parsing and validation failures shall be detected during startup before the editor begins interactive rendering.
- Usability: Built-in themes shall expose stable, human-readable names suitable for CLI selection and user-facing error messages.
- Maintainability: The theme model shall be extensible enough to add more UI style keys and future syntax style keys without changing the default-style inheritance behavior.

## Acceptance Criteria

- [ ] **AC-001**: Starting the editor without a `--theme` flag activates the built-in `Friday Night` ANSI theme.
- [ ] **AC-002**: Starting the editor with `--theme "Tokyo Night"` activates the built-in Tokyo Night theme by matching its theme name.
- [ ] **AC-003**: Each built-in theme is defined in TOML and is parsed from a statically included source during startup.
- [ ] **AC-004**: A theme fails startup validation if it omits the theme name, palette section, default style section, `ui` section, or `syntax` section.
- [ ] **AC-005**: A cell with no element-specific style renders using only the theme default style.
- [ ] **AC-006**: A UI or syntax style that sets only foreground color inherits any unspecified properties, including background color, from the default style.
- [ ] **AC-007**: A style entry that names an undefined palette color causes theme loading to fail with a clear startup error.
- [ ] **AC-008**: Rendering code requests one predefined UI key or predefined syntax key for an element and receives the final style produced from that style plus the default style.
- [ ] **AC-009**: The theme system rejects attempts to define or resolve styling through arbitrary stacked named styles.
- [ ] **AC-010**: A theme whose palette contains only ANSI color values is classified as 256-color.
- [ ] **AC-011**: A theme whose palette contains at least one true color value is classified as true color.
- [ ] **AC-012**: The same theme system can describe UI styles today and syntax styles when syntax highlighting is introduced.
- [ ] **AC-013**: A theme that contains an unknown key in the `ui` or `syntax` section fails validation during startup.

## Out of Scope

- Runtime theme switching after the editor has started
- Loading user themes from external filesystem paths
- Terminal capability negotiation beyond theme classification
- Syntax parser or lexer implementation
- Hierarchical style lookup or fallback between named styles
- Arbitrary stacking or composition of multiple named styles for one rendered element
- Theme-defined custom UI or syntax keys beyond the predefined urvim schema

## Assumptions

- Existing rendering code can adopt a theme-provided default style and request one predefined UI or syntax style per rendered element.
- The CLI already has a startup configuration path where a `--theme` option can be validated before entering the main editor loop.
- ANSI color values and true color values already have, or can gain, distinct representations in the editor color model.
- Built-in themes may use display names with spaces as long as CLI selection matches the theme name consistently.
- The predefined UI and syntax keys are owned by urvim's schema, not by theme authors.

## Dependencies

- Existing CLI argument parsing
- Existing terminal color/style rendering primitives
- A startup initialization path that can parse and validate embedded theme TOML before UI rendering begins
