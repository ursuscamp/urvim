# Unified Highlight Themes - Technical Design

## Architecture Overview
The theme system will move from a split model to a single highlight-name model.

Today, the resolver produces separate UI fields and syntax-tag styles. In the new design, every styled element is represented by a hierarchical highlight name and resolved through the same parent-fallback rules.

The key architectural changes are:

- remove the closed `ui` style struct from the public theme model
- replace the separate syntax-only map with one unified highlight map
- keep hierarchical lookup semantics for all named styles
- preserve the existing render-time style overlay behavior in the renderer

The theme document will still have a palette and a default style, but there will be only one named-style table. Built-in themes will be rewritten to use `ui.*` and `syntax.*` names within that unified table.

## Interface Design
### Theme document shape
The raw theme document will be a single TOML structure with these logical sections:

- `name`
- `palette`
- `default`
- `highlights`

`highlights` is the unified table of named styles. Its keys are hierarchical highlight names such as `ui.status_bar`, `ui.window.active_line`, and `syntax.comment.todo`.

### Core lookup API
The resolved theme should expose a single lookup entry point for all highlight names:

```rust
pub fn highlight_style_for_tag(&self, tag: &Tag) -> Style
```

This API returns the resolved style for the closest defined ancestor of `tag`, or the theme default style when nothing in the chain is defined.

Existing syntax-oriented callers may keep using `Tag` as the hierarchical name type, because it already matches the dot-separated lookup model.

### Resolution rules

- exact match wins
- otherwise, fall back to the nearest parent name
- do not combine ancestor styles with descendant styles during lookup
- apply the returned style later with `Style::overlay` at render time, as the current renderer already does

## Data Models
### Raw theme model
The parsed TOML model will be updated from:

- `default`
- `ui`
- `syntax`

to:

- `default`
- `highlights`

The `highlights` field will be a map from hierarchical name strings to raw style definitions.

### Resolved theme model
The resolved theme will retain:

- theme name
- color kind
- default style

The resolved highlight data will become a single map keyed by validated hierarchical names. The public theme model should no longer expose separate `ui` and `syntax` collections.

### Naming conventions
The built-in themes will use a consistent prefix scheme:

- `ui.*` for editor chrome
- `syntax.*` for syntax highlighting

Recommended UI names include:

- `ui.status_bar`
- `ui.status_bar.modified_marker`
- `ui.selection`
- `ui.window`
- `ui.window.active_line`
- `ui.window.gutter`
- `ui.tab.active`
- `ui.tab.inactive`
- `ui.tab.scroll_indicator`
- `ui.window.split_border`
- `ui.window.split_border.resize`

Recommended syntax names continue to follow the current syntax hierarchy under `syntax.*`, such as:

- `syntax.comment`
- `syntax.comment.todo`
- `syntax.string`
- `syntax.string.interpolation`

## Key Components
### `src/theme/schema.rs`
Owns the TOML-facing raw theme schema.

Responsibilities:

- parse the unified `highlights` table
- validate raw style fields and palette references
- reject unknown fields and malformed highlight names

### `src/theme/loader.rs`
Resolves raw theme data into runtime theme data.

Responsibilities:

- resolve palette entries and theme kind
- resolve `default`
- resolve each named highlight into a style
- validate hierarchical highlight names using the existing tag parser

### `src/theme/model.rs`
Owns the resolved runtime theme model.

Responsibilities:

- store the unified highlight map
- provide hierarchical style lookup
- expose theme metadata needed by the rest of the editor

### Rendering call sites
Consumers such as `src/window/mod.rs`, `src/window/view.rs`, and `src/status_bar.rs` will request styles by hierarchical name instead of reading from dedicated UI fields.

Responsibilities:

- choose the correct highlight name for each rendered region
- overlay the returned highlight style onto the current lower style
- preserve the current order of composition for buffer text, active line background, selections, and status text

### Documentation files
`docs/syntax/highlighting.md` and `docs/syntax/tags.md` will describe the new unified naming model.

Responsibilities:

- explain that both UI and syntax styles use the same hierarchical lookup rules
- document the `ui.*` and `syntax.*` naming convention
- clarify that parent fallback is a lookup rule, not style inheritance by merge

## User Interaction
Theme authors will edit one unified highlight table instead of two separate style sections.

Practical effects:

- a theme can define broad styles once and refine them with deeper names
- UI styles can be organized by component, just like syntax styles are organized by token category
- built-in themes will be easier to scan because UI and syntax groups can be visually separated with comments while still living in the same table

Existing editor behavior should remain intuitive:

- syntax spans still choose a style by tag
- the active line still renders as a background overlay behind the current buffer line
- status bars, tabs, gutters, and split borders still use theme-driven styles

## External Dependencies
No new external libraries are required.

The design depends on:

- the existing TOML parser
- the existing theme palette and color resolution code
- the existing `Tag` parser and parent-chain iterator
- the existing `Style::overlay` composition behavior in the terminal renderer

## Error Handling
Expected failures should continue to fail fast and clearly:

- malformed TOML should report a parse error
- invalid color literals should report palette resolution errors
- unknown palette references should report the offending key and reference
- invalid highlight names should report the invalid name using the existing hierarchical-name validation path
- references to removed legacy `ui` or `syntax` sections should fail during parsing because the new schema does not accept them

When a highlight name is missing:

- resolve the nearest parent if one exists
- otherwise return the theme default style

The lookup path must not attempt to synthesize a partial style by merging ancestor definitions.

## Security
The theme rewrite does not introduce new security-sensitive behavior.

The only relevant checks are input validation for theme files and palette references. The design should continue to reject malformed or unexpected theme input rather than trying to recover silently.

## Configuration
No new editor configuration options are required for the theme model change.

The existing active-line toggle remains unchanged. It continues to decide whether the current buffer line should be painted with the active-line highlight.

## Component Interactions
1. Theme TOML is parsed into the raw theme schema.
2. The loader resolves palette colors and converts each named highlight into a resolved style.
3. The runtime theme stores one unified highlight map keyed by hierarchical names.
4. Rendering code asks the theme for a highlight style using the relevant `Tag`.
5. The renderer overlays that style onto lower styles such as the default buffer style, syntax span style, or line background.

Important interaction detail:

- lookup is unified and hierarchical
- composition is still done by the renderer, not by theme lookup

This distinction preserves the current “choose one named style, then overlay it where needed” behavior while removing the old UI/syntax split.

## Platform Considerations
The theme refactor should remain platform-neutral.

Terminal capability handling, ANSI-vs-true-color detection, and screen rendering behavior are unchanged. The only change is how named highlight styles are organized and resolved before being written to the terminal.
