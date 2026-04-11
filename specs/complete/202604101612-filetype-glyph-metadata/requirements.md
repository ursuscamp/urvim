# Filetype Glyph Metadata

## Summary

Add optional glyph metadata to syntax definitions so each filetype can advertise a language icon and an optional default glyph color. Add an editor configuration option for advanced glyph capabilities, currently limited to `nerdfont`, so the application can decide when to render glyphs. When a syntax definition provides a glyph and the active configuration enables the required capability, the editor should render the icon in the tab bar and status bar alongside the existing language label.

## Problem Statement

urvim currently uses syntax metadata to identify filetypes and show human-readable labels, but it does not expose filetype-specific icon metadata for compact UI surfaces. That makes the tab bar and status bar less visually scannable, especially for users who rely on nerdfont icons to recognize languages quickly. The editor also needs a clean way to opt into advanced glyph rendering through configuration without making glyphs mandatory for every syntax.

## User Stories

- As a user with nerdfont support, I want recognizable language icons in the tab bar and status bar, so that I can identify filetypes faster.
- As a user without nerdfont support, I want the editor to keep showing the existing text labels, so that nothing breaks when icons are unavailable.
- As a syntax maintainer, I want to declare a default glyph and glyph color in syntax metadata, so that the UI can render consistent icons without hard-coded per-language logic.
- As a user, I want to enable advanced glyph support from editor configuration, so that I can opt into nerdfont icons without changing syntax files.

## Functional Requirements

- [ ] **REQ-001**: Syntax metadata shall support an optional glyph value for each syntax definition.
- [ ] **REQ-002**: Syntax metadata shall support an optional glyph color value for each syntax definition.
- [ ] **REQ-003**: Editor configuration shall support an optional set of advanced glyph capabilities.
- [ ] **REQ-004**: The advanced glyph capability set shall include `nerdfont` as the initial supported value.
- [ ] **REQ-005**: The configuration loader shall accept configurations that omit the advanced glyph capability set.
- [ ] **REQ-006**: The configuration loader shall reject unknown advanced glyph capability values.
- [ ] **REQ-007**: Built-in syntax definitions shall provide glyph metadata only for languages that have a suitable default icon and optional default color.
- [ ] **REQ-008**: When an active syntax defines a glyph and the required advanced glyph capability is enabled in configuration, the editor shall render that glyph in the tab bar and status bar.
- [ ] **REQ-009**: When a glyph color is defined, the editor shall use that color for the glyph foreground without changing the surrounding label styling.
- [ ] **REQ-010**: When glyph metadata is absent or the required advanced glyph capability is not enabled, the editor shall continue rendering the existing text labels with no icon.
- [ ] **REQ-011**: The addition of glyph metadata and glyph-related configuration shall not change filetype detection, syntax highlighting, or existing tab and status text content apart from the optional icon display.

## Non-Functional Requirements

- Compatibility: Existing syntax definitions without glyph metadata shall continue to load successfully, and existing configurations without advanced glyph options shall continue to work.
- Usability: Glyph rendering shall remain legible and shall not break tab bar or status bar layout on narrow terminals.
- Reliability: Missing glyph colors or unsupported glyph capabilities shall fall back cleanly to the existing text-only presentation.
- Performance: Glyph metadata lookups shall not introduce noticeable overhead in normal tab bar or status bar rendering.

## Acceptance Criteria

- [ ] **AC-001**: A filetype with a glyph, a glyph color, and `nerdfont` enabled shows the glyph in both the tab bar and status bar.
- [ ] **AC-002**: The rendered glyph uses its configured color when a glyph color is provided.
- [ ] **AC-003**: A filetype with a glyph but without the required advanced glyph capability enabled renders the existing text label with no icon.
- [ ] **AC-004**: A filetype without glyph metadata renders exactly as it does today in the tab bar and status bar.
- [ ] **AC-005**: A syntax definition that declares an unknown advanced glyph capability is rejected by the loader.
- [ ] **AC-006**: Existing built-in syntax definitions that do not declare glyph metadata continue to load and display normally.

## Out of Scope

- Adding advanced glyph capabilities beyond `nerdfont`.
- Allowing users to override per-language glyphs from runtime configuration.
- Rendering filetype glyphs in other UI surfaces beyond the tab bar and status bar.
- Changing filetype detection, syntax highlighting, or comment toggling behavior.

## Assumptions

- Syntax metadata will store glyph information alongside existing filetype metadata.
- Tab bar and status bar rendering will continue to use the syntax display name as the text label.
- The application already has, or will have, a single runtime decision about whether `nerdfont` glyph rendering is enabled.
- Sensible per-language glyph and color defaults can be chosen for the built-in syntaxes without changing filetype behavior.

## Dependencies

- Renderer support for drawing a glyph with an optional foreground color.
- A runtime configuration gate for enabling or disabling `nerdfont` glyph rendering.
- Default glyph and color choices for built-in syntax definitions.
