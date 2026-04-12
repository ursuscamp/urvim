# Todo Comment Highlighting - Technical Design

## Architecture Overview
Todo highlighting will be a derived styling pass layered on top of the existing syntax-aware render pipeline. The editor already identifies comment regions through syntax spans, and the new feature will inspect only those comment regions for configured task markers such as `TODO` and `FIXME`.

The feature is intentionally passive:
- it does not change buffer contents
- it does not alter syntax detection
- it does not persist any marker state

Marker scanning will run on demand during rendering of visible lines. The render path will inspect syntax spans after they have been resolved for a line, scan only the comment regions that are about to be drawn, and derive marker styling immediately before the screen is written.

Rendering will use three inputs:
- the active syntax spans for each line
- the configured marker list
- the active theme's syntax tag styles

The marker pass will split comment text into smaller render segments when a configured marker is present, then assign a marker-specific tag to the matched segment. Theme resolution will use the existing hierarchical tag lookup so marker styles can inherit from broader comment styles when needed.

## Interface Design

### Configuration
Add a config field for the highlighted marker list.

```toml
todo_markers = ["TODO", "FIXME", "BUG", "NOTE"]
```

Behavior:
- when omitted, the built-in default marker list is used
- when present, the configured list replaces the default list
- marker matching is case-sensitive
- marker values must be standalone word tokens

### Theme Styles
Themes will expose syntax tags for marker-specific styling using the existing syntax style map.

Recommended tag names:
- `comment.todo`
- `comment.fixme`
- `comment.bug`
- `comment.note`

These tags remain compatible with existing syntax inheritance:
- `comment.todo` can inherit from `comment`
- themes that only define `comment` still render highlighted comment text normally

For custom markers, the literal match text and the theme tag are separate:
- the marker matcher uses the configured marker text exactly as written
- the theme tag is derived by normalizing the marker text into a valid lowercase tag segment
- the resolved tag should stay under the `comment.*` hierarchy so it can inherit from broader comment styles

Normalization rule for custom marker tags:
- convert the marker text to lowercase
- replace any non-`[a-z0-9_]` character with `_`
- collapse repeated separators if needed so the final segment is a valid tag component
- if the normalized result is empty or invalid, the marker configuration should be rejected

## Data Models

### Marker Configuration
```rust
pub struct TodoMarkerConfig {
    pub markers: Vec<SmolStr>,
}
```

Fields:
- `markers`: ordered list of literal marker strings to search for in comment text

Constraints:
- entries must be non-empty
- entries must resolve to valid standalone word tokens
- entries are matched case-sensitively

### Marker Match
```rust
pub struct TodoMarkerMatch {
    pub start_byte: usize,
    pub end_byte: usize,
    pub marker: SmolStr,
}
```

Fields:
- `start_byte` and `end_byte` mark the literal marker substring within a line
- `marker` stores the configured marker text that matched

### Marker Style Resolution
Marker styles are resolved through tag lookup, not by storing literal colors in the buffer.

Resolution order:
1. marker-specific tag, such as `comment.todo`
2. broader comment tag, such as `comment`
3. theme default style

## Key Components

### Comment Marker Scanner
Responsibility:
- scan a line's comment spans for configured markers
- enforce standalone-word matching
- return marker matches in left-to-right order
- run only when a line is being rendered

Dependencies:
- existing syntax spans for a line
- configured marker list

Public behavior:
- accepts a line of text and the comment-only byte ranges for that line
- returns zero or more literal matches
- ignores text outside comment spans

### Marker Style Resolver
Responsibility:
- map a matched marker to the appropriate theme tag
- resolve a fallback style when a marker-specific tag is not present

Dependencies:
- active theme
- theme syntax tag hierarchy

Public behavior:
- returns a resolved `Style` for a marker
- never fails the render path if a marker-specific style is missing
- uses the normalized custom-marker tag for configured custom markers
- uses the fixed built-in tags for the default marker set

### Render Overlay
Responsibility:
- split comment chunks at marker boundaries
- apply marker-specific styles while preserving the surrounding comment style

Dependencies:
- current line's syntax spans
- marker scan results
- theme style resolver

Public behavior:
- preserves the existing render shape outside highlighted marker text
- leaves cursor positioning, editing, and buffer state untouched

## User Interaction
The feature is visually passive and requires no new editor action.

User-visible behavior:
- markers inside comments appear with marker-specific styling
- custom marker lists change which marker literals receive highlighting
- non-comment text is unaffected
- syntax highlighting can still be disabled globally through existing config, which also disables marker highlighting because comment regions are no longer available

## External Dependencies
- Existing syntax-aware comment detection
- Existing theme loading and syntax tag resolution
- Existing config loading and TOML parsing
- Existing render chunk and screen output pipeline

No new network, filesystem, or external service dependencies are required.

## Error Handling
Expected failures and recovery behavior:
- invalid configured marker values should fail config validation with a clear startup error
- missing marker-specific theme tags should fall back to broader comment styling or the theme default style
- an empty or missing marker list should fall back to the built-in default markers
- syntax-disabled rendering should skip marker detection rather than producing partial or stale styling

The feature should not surface runtime errors during ordinary rendering because marker highlighting is derived data.

## Security
This feature does not introduce new security-sensitive behavior.

- it does not execute marker text
- it does not parse untrusted content as code
- it does not read or write any additional files
- it does not expand the attack surface beyond existing syntax highlighting and config loading

## Configuration
The default configuration behavior should be:

```toml
todo_markers = ["TODO", "FIXME", "BUG", "NOTE"]
```

Configuration requirements:
- the field is optional
- if provided, it replaces the default marker list
- values are literal case-sensitive markers
- the marker list should be ordered so future behavior can preserve priority if two markers overlap
- custom marker values must normalize to a valid theme tag segment before they are accepted

Theme configuration requirements:
- marker-specific styles are added to the existing syntax style map
- theme authors can define only the marker styles they care about
- missing marker styles inherit through the current tag hierarchy

## Component Interactions
1. The buffer produces syntax spans for a visible line.
2. The render layer filters those spans to comment regions.
3. The todo scanner searches only the comment regions using the configured marker list.
4. The marker style resolver maps each hit to a theme tag and resolves a style.
5. The render chunk builder splits the comment segment where needed and applies the resolved marker style to the matching subspan.
6. The screen renderer writes the final styled chunks without changing buffer state.

## Platform Considerations
- Matching should be byte-accurate within UTF-8 lines, but marker literals themselves are expected to be ASCII word tokens.
- The standalone-word rule should use stable boundary checks so behavior is consistent across terminals and platforms.
- The feature should not depend on terminal capabilities beyond the current style system.
- Existing ANSI and true-color theme behavior should continue to work unchanged.
