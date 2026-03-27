# Theme System - Technical Design

## Architecture Overview

The theme system introduces a startup-loaded theme registry and a small styling abstraction layer above `terminal::Style`. Themes are defined as TOML documents, parsed and validated during startup, then exposed as resolved runtime themes that can be selected by name from the CLI.

The design keeps terminal escape generation unchanged. The new logic lives one layer higher:

- a `theme` module owns parsing, validation, built-in theme registration, and style resolution
- startup code loads the built-in TOML themes, validates them, selects the active theme, and passes it into the editor UI
- UI renderers request one concrete predefined UI key or syntax key, and the theme returns a final `terminal::Style` produced by layering that named partial style onto the theme default style

This architecture separates:

- theme authoring format: TOML with palette names plus `default`, `ui`, and `syntax` sections
- runtime theme model: validated palette-backed styles resolved to concrete `terminal::Color` and `terminal::Style`
- rendering behavior: start from the default style and apply exactly one predefined style override for the rendered element

### Flow

```text
Startup
  -> include built-in TOML theme files
  -> parse raw theme documents
  -> validate palette/default/ui/syntax rules
  -> classify each theme as ANSI or true color
  -> register themes by name
  -> select active theme from --theme or default to Friday Night

UI rendering
  -> renderer asks theme for default style or one predefined ui/syntax style
  -> theme resolves palette references at startup, not render time
  -> theme layers the requested partial style on top of the default style
  -> final terminal::Style is written to screen cells
```

## Interface Design

The public interfaces stay shallow and centered around loading themes and resolving final styles.

### CLI

`src/main.rs` will extend the existing `Cli` parser:

```rust
#[derive(Parser)]
struct Cli {
    #[arg(long)]
    theme: Option<String>,
    files: Vec<std::path::PathBuf>,
}
```

Behavior:

- if `--theme` is omitted, startup selects the built-in `Friday Night` theme
- if `--theme` is present, startup looks up an exact theme-name match
- unknown theme names fail startup with a user-facing error

### Theme loading

New public API in `src/theme/mod.rs`:

```rust
pub struct ThemeRegistry { ... }

impl ThemeRegistry {
    pub fn load_builtin() -> Result<Self, ThemeLoadError>;
    pub fn get(&self, name: &str) -> Option<&Theme>;
    pub fn default_theme(&self) -> &Theme;
    pub fn names(&self) -> Vec<&str>;
}
```

Responsibilities:

- parse all statically included TOML theme files
- validate required sections and references
- expose themes by their declared names
- remember the built-in default theme

### Theme style resolution

The theme API uses typed keys instead of caller-provided strings.

```rust
pub enum UiStyleKey {
    StatusBar,
    TabActive,
    TabInactive,
    TabScrollIndicator,
    Gutter,
    Window,
}

pub enum SyntaxStyleKey {
    Comment,
    Constant,
    Function,
    Keyword,
    Number,
    Operator,
    Punctuation,
    String,
    Type,
    Variable,
}

pub struct Theme { ... }

impl Theme {
    pub fn name(&self) -> &str;
    pub fn kind(&self) -> ThemeKind;
    pub fn default_style(&self) -> Style;
    pub fn ui_style(&self, key: UiStyleKey) -> Style;
    pub fn syntax_style(&self, key: SyntaxStyleKey) -> Style;
}
```

Behavior:

- `ui_style` returns the final style for one predefined UI element by layering that partial style onto the default style
- `syntax_style` returns the final style for one predefined syntax element by layering that partial style onto the default style
- the key sets are closed and versioned by the codebase, not by theme authors
- no API supports stacking multiple named styles for a single rendered element

### Render-facing usage

Existing renderers currently build `terminal::Style` directly. They will instead receive access to the active `Theme` and use helper methods such as:

```rust
let style = theme.ui_style(UiStyleKey::StatusBar);
screen.write_string(row, col, style, text);
```

For future syntax rendering:

```rust
let style = theme.syntax_style(SyntaxStyleKey::Keyword);
```

This keeps `Screen`, `Cell`, and terminal output unchanged.

## Data Models

The design uses three layers of data: deserialized TOML input, validated intermediate values, and resolved runtime theme data.

### Raw TOML models

These types mirror the file format and are only used during parsing.

```rust
pub struct RawTheme {
    pub name: String,
    pub palette: BTreeMap<String, RawColorValue>,
    pub default: RawStyle,
    pub ui: RawUiStyles,
    pub syntax: RawSyntaxStyles,
}

#[serde(deny_unknown_fields)]
pub struct RawUiStyles {
    pub status_bar: RawStyle,
    pub tab_active: RawStyle,
    pub tab_inactive: RawStyle,
    pub tab_scroll_indicator: RawStyle,
    pub gutter: RawStyle,
    pub window: RawStyle,
}

#[serde(deny_unknown_fields)]
pub struct RawSyntaxStyles {
    pub comment: RawStyle,
    pub constant: RawStyle,
    pub function: RawStyle,
    pub keyword: RawStyle,
    pub number: RawStyle,
    pub operator: RawStyle,
    pub punctuation: RawStyle,
    pub string: RawStyle,
    pub type_: RawStyle,
    pub variable: RawStyle,
}

pub struct RawStyle {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub underline_color: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub double_underline: Option<bool>,
    pub dim: Option<bool>,
    pub reverse: Option<bool>,
    pub blink: Option<bool>,
    pub strikethrough: Option<bool>,
    pub overline: Option<bool>,
}
```

Constraints:

- `name` is required and must be non-empty
- `palette` is required and must be non-empty
- `default`, `ui`, and `syntax` sections are required
- `fg`, `bg`, and `underline_color` values in all style sections are palette keys, not literal colors
- the `ui` and `syntax` sections are closed schemas with no extra fields

### Color values

Palette values support two input forms:

- integer ANSI value: `0` through `255`
- hex string RGB value: `"#RRGGBB"`

Validation rules:

- ANSI integers outside `0..=255` are rejected
- RGB strings must be exactly six hexadecimal digits after `#`
- theme kind is `Ansi256` only if every palette entry is ANSI
- theme kind is `TrueColor` if any palette entry is RGB

### Resolved runtime models

```rust
pub enum ThemeKind {
    Ansi256,
    TrueColor,
}

pub struct StyleOverride {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub underline_color: Option<Color>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub double_underline: Option<bool>,
    pub dim: Option<bool>,
    pub reverse: Option<bool>,
    pub blink: Option<bool>,
    pub strikethrough: Option<bool>,
    pub overline: Option<bool>,
}
```

`StyleOverride` is intentionally partial. Every field means “override this property from the default style if present”.

The runtime `Theme` stores:

- `name: String`
- `kind: ThemeKind`
- `default_style: Style`
- `ui_styles: UiStyles`
- `syntax_styles: SyntaxStyles`

`UiStyles` and `SyntaxStyles` mirror the closed key sets used by the TOML schema, but store fully resolved `Style` values rather than partial overrides.

Resolution strategy:

- convert `default` into a full `Style`
- resolve each `ui` and `syntax` `RawStyle` into a `StyleOverride`
- merge the override onto `default_style`
- store the fully resolved final `Style` for each predefined UI and syntax key

Precomputing final styles at startup keeps render-time lookups simple and avoids repeated merging.

## Key Components

### `src/theme/mod.rs`

Public module entry point.

Responsibilities:

- export theme types
- expose built-in theme loading
- define `ThemeLoadError`

Dependencies:

- `src/theme/loader.rs`
- `src/theme/parser.rs`
- `src/theme/registry.rs`
- `crate::terminal::style`

### `src/theme/parser.rs`

Owns TOML deserialization into raw theme types.

Responsibilities:

- deserialize embedded TOML strings
- report syntax and schema errors with theme source context

### `src/theme/loader.rs`

Owns validation and resolution from raw themes to runtime themes.

Responsibilities:

- validate required `default`, `ui`, and `syntax` sections
- reject unknown fields in the `ui` and `syntax` sections
- resolve palette references into concrete `Color` values
- classify theme kind
- convert `default` into a full `Style`
- convert each predefined UI and syntax style into a final `Style` by layering it over the default style

### `src/theme/registry.rs`

Stores all resolved themes.

Responsibilities:

- load built-in theme definitions from `include_str!`
- reject duplicate theme names
- expose lookup by theme name
- expose the configured built-in default theme

### `src/theme/builtin/*.toml`

Statically included built-in theme sources:

- `friday-night.toml`
- `saturday-morning.toml`
- `rose-pine.toml`
- `dracula.toml`
- `tokyo-night.toml`
- `catppuccin.toml`

Responsibilities:

- define the initial palette plus `default`, `ui`, and `syntax` sections
- use only the predefined closed key sets
- provide enough UI style keys for current editor surfaces
- provide initial syntax style keys so the format is ready for later syntax highlighting work

### UI integration points

The first implementation will update the current hard-coded style producers in:

- `src/status_bar.rs`
- `src/tab_group.rs`
- `src/window/*` render paths where buffer, gutter, cursor line, and related UI styles are chosen

These components will no longer hard-code specific colors. Instead they will request predefined UI style keys such as:

- `UiStyleKey::StatusBar`
- `UiStyleKey::TabActive`
- `UiStyleKey::TabInactive`
- `UiStyleKey::TabScrollIndicator`
- `UiStyleKey::Gutter`
- `UiStyleKey::Window`

The same closed-key approach applies to future syntax rendering through `SyntaxStyleKey`.

## User Interaction

From the user perspective, the interaction is startup-only:

1. User runs `urvim` without `--theme`
2. Editor loads built-in themes and selects `Friday Night`
3. UI renders using the active theme

Or:

1. User runs `urvim --theme "Tokyo Night"`
2. Editor loads built-in themes
3. Startup validates that `Tokyo Night` exists
4. UI renders using the selected theme

Failure path:

1. User runs `urvim --theme missing`
2. Startup prints a clear error listing the unknown theme name
3. Process exits before entering interactive mode

## External Dependencies

This feature requires adding a TOML deserialization dependency.

Expected additions:

- `serde` with `derive` for TOML deserialization
- `toml` for parsing embedded theme documents

No new runtime system dependencies are needed beyond Rust crates already used by the project.

## Error Handling

| Scenario | Behavior |
| --- | --- |
| Built-in TOML syntax error | Startup fails with theme source name and parse error |
| Missing `name`, `palette`, `default`, `ui`, or `syntax` section | Startup fails with validation error |
| Duplicate built-in theme names | Startup fails with duplicate-name error |
| Style color refers to unknown palette key | Startup fails with theme name, section, style key, and missing palette key |
| Style directly uses a literal color where a palette key is required | Startup fails validation |
| Unknown CLI theme name | Startup fails before entering terminal mode |
| Theme contains an unknown `ui` or `syntax` field | Startup fails validation |

Built-in theme failures are fatal because the editor cannot safely continue with a partially valid built-in registry.

## Security

The feature has low security impact because themes are built into the binary in the initial implementation.

Relevant safeguards:

- strict input validation during TOML parsing
- no filesystem theme loading in the initial version
- no dynamic code execution or shell interaction

If external theme loading is added later, path handling and untrusted-file validation will need separate review.

## Configuration

The only new user-facing configuration is:

- `--theme <name>`: choose a built-in theme by declared theme name

Default behavior:

- if omitted, use the built-in `Friday Night` theme

Internal configuration decisions:

- built-in theme list is fixed at compile time
- default theme name is fixed to `Friday Night`
- the `ui` and `syntax` schema keys are fixed by urvim, not by theme authors
- every final named style is derived from the theme default style

## Component Interactions

```text
main.rs
  -> ThemeRegistry::load_builtin()
  -> choose theme by CLI name or default
  -> pass active Theme into root UI objects

Theme
  -> default_style()
  -> ui_style(UiStyleKey::StatusBar)
  -> syntax_style(SyntaxStyleKey::Keyword)
  -> final terminal::Style

StatusBar / TabGroup / Window renderers
  -> request one predefined UI style key per rendered element
  -> write final Style into Screen cells

Future syntax renderer
  -> request one predefined syntax style key per token class

Screen
  -> unchanged storage of text + terminal::Style
Terminal
  -> unchanged ANSI escape emission from terminal::Style
```

The important boundary is that theming stops at `terminal::Style`. Terminal output remains unaware of palette names, schema keys, or inheritance rules.

## Platform Considerations

The theme data model is platform-neutral because it resolves to the existing terminal style types already used across urvim.

Terminal-specific considerations:

- ANSI-only themes remain compatible with terminals limited to 256 colors
- true color themes rely on the existing RGB escape generation already supported by `terminal::Style`
- no additional platform branching is required for macOS, Linux, or other Unix-like terminals in this phase
