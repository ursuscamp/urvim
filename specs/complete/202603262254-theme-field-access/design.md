# Theme Field Access Simplification - Technical Design

## Architecture Overview

`Theme` is the resolved runtime styling object used by rendering code. Today it exposes UI and syntax styles through accessor methods. This design replaces those accessors with direct fields named `ui` and `syntax`, while leaving the resolved style data and theme-loading pipeline unchanged.

The change is intentionally shallow:

- The theme loader continues to resolve `UiStyles` and `SyntaxStyles` exactly as it does today.
- Rendering code reads those collections directly from `Theme`.
- Existing `default_style()` and `kind()` accessors remain unchanged.

## Interface Design

### `Theme`

The public runtime theme type will expose:

- `pub name: String` remains private; no change to the theme identity API.
- `pub kind: ThemeKind` remains private; no change to the theme color-kind API.
- `pub default_style: Style` remains private; no change to the default-style API.
- `pub ui: UiStyles`
- `pub syntax: SyntaxStyles`

The following methods are removed from the public API:

- `ui_style(&self, key: UiStyleKey) -> Style`
- `syntax_style(&self, key: SyntaxStyleKey) -> Style`

Callers will read the underlying collections directly and access the concrete style fields on `UiStyles` and `SyntaxStyles`.

### `Theme::new`

`Theme::new` remains the constructor for resolved themes, but its parameters will be updated to match the renamed fields:

- `name`
- `kind`
- `default_style`
- `ui`
- `syntax`

## Data Models

### `Theme`

`Theme` remains a resolved theme with the same logical contents:

- `name: String`
- `kind: ThemeKind`
- `default_style: Style`
- `ui: UiStyles`
- `syntax: SyntaxStyles`

The semantic meaning of the collections does not change. Only the field names and access pattern change.

### `UiStyles` and `SyntaxStyles`

These existing resolved style collections remain the same data models. Their fields remain the canonical storage for the resolved UI and syntax styles. No schema or value changes are introduced.

## Key Components

### Theme model

Responsibilities:

- Own the resolved theme data used at runtime.
- Provide direct access to UI and syntax style collections.
- Continue exposing theme metadata such as `kind()` and `default_style()`.

Dependencies:

- `ThemeKind`
- `Style`
- `UiStyles`
- `SyntaxStyles`

### Theme loader

Responsibilities:

- Resolve raw theme documents into `Theme`.
- Populate the renamed `ui` and `syntax` fields with the same resolved values currently used by the accessor methods.

Dependencies:

- `RawTheme`
- `UiStyles`
- `SyntaxStyles`
- palette and style resolution helpers

### Rendering call sites

Responsibilities:

- Replace `theme.ui_style(...)` and `theme.syntax_style(...)` usage with direct field access.
- Continue selecting the same concrete styles for gutters, tabs, status bars, windows, and syntax highlighting.

Dependencies:

- `Theme`
- `UiStyles`
- `SyntaxStyles`

## User Interaction

No user-facing interaction changes are expected. The refactor is internal to the editor codebase and should preserve rendered output.

## External Dependencies

No external dependencies change. Theme parsing and rendering continue to rely on the existing in-repo theme schema and terminal styling types.

## Error Handling

The refactor should not introduce new runtime error paths. Existing theme resolution errors, invalid palette handling, and missing-section validation remain unchanged.

If a call site is missed during the migration, the compiler should surface the outdated method reference or field name, making the change safe to complete incrementally.

## Security

No security-sensitive behavior changes. Theme data remains local configuration data and does not affect authentication, secrets, or trust boundaries.

## Configuration

No configuration changes are required. Existing theme files and theme registry inputs should continue to load without modification.

## Component Interactions

1. Theme loading resolves raw TOML into `UiStyles` and `SyntaxStyles`.
2. `Theme::new` stores those collections directly in the renamed public fields.
3. Rendering components read styles directly from `theme.ui` and `theme.syntax`.
4. `default_style()` continues to provide the base style for inherited rendering.

## Platform Considerations

The change is platform-neutral. It affects Rust API shape and call-site ergonomics only, with no terminal-specific or OS-specific behavior.
