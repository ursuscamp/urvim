# Indent Scope Tracking - Technical Design

## Architecture Overview
Indent scope tracking is introduced as a sibling cache to syntax highlight spans in the buffer. Both caches share lifecycle events (invalidation and rebuild), but indent scopes remain independent data so future consumers (folding and indentation guides) can read structure without coupling to highlight tags.

High-level flow:
1. Buffer edits trigger existing syntax-cache invalidation.
2. The same invalidation step marks indent scopes stale.
3. The syntax rebuild path tokenizes/highlights as it does today and also recomputes indent scopes from current buffer lines.
4. Rebuild stores both updated highlight cache and indent scope cache atomically for the buffer.

## Interface Design
Expose a public buffer-facing API for indent scopes while keeping computation encapsulated in syntax cache rebuild internals.

Proposed public interfaces (names illustrative):

- `Buffer::indent_scopes() -> &[IndentScope]`
- `Buffer::line_indent_scopes(line: usize) -> &[IndentScopeId]`
- `Buffer::indent_scope_cache_is_stale() -> bool`

Behavioral contract:
- Read APIs return the most recent rebuilt cache and are consistent with current syntax-cache generation.
- Per-line lookup returns containing scopes ordered outer-to-inner for predictable folding/guide traversal.
- Callers do not mutate scope cache directly.

## Data Models
### `IndentScope`
Represents one normalized indentation scope.

Fields:
- `id`: stable scope identifier within the cache generation.
- `start_line: usize`: inclusive opening line.
- `end_line: usize`: inclusive closing line (or EOF line when no explicit closer exists).
- `indent_width: usize`: normalized visual indentation width of boundary lines.

Constraints:
- `start_line < end_line`
- At least one inside line exists (`end_line - start_line >= 2`).
- Boundary lines have equal normalized `indent_width`.

### `IndentScopeCache`
Stores all scope records and reverse lookup indexes.

Fields:
- `scopes: Vec<IndentScope>`
- `line_to_scopes: Vec<Vec<IndentScopeId>>`
- `generation` or equivalent cache version metadata aligned with syntax cache generation.

Constraints:
- `line_to_scopes.len()` matches buffer line count at rebuild time.
- Every scope id in `line_to_scopes` references an existing scope record.

## Key Components
### Syntax rebuild coordinator
Responsibilities:
- Trigger indent scope recomputation whenever syntax cache rebuild runs.
- Ensure highlight and indent cache results are committed together.

### Indent normalization helper
Responsibilities:
- Compute normalized visual indent width from each line's leading whitespace.
- Expand tabs using configured tab width, defaulting to `4` when configuration is unavailable.

### Scope builder
Responsibilities:
- Scan lines once in order.
- Build nested scopes by tracking active opening candidates keyed by normalized indent width.
- Emit scope records when a closing boundary is encountered or EOF finalization allows closure.
- Enforce inside-line rules where whitespace-only lines count as non-empty and zero-length lines do not.

### Buffer cache accessors
Responsibilities:
- Provide read-only access to scope records and per-line containing-scope lookup.
- Hide internal storage details from feature consumers.

## User Interaction
There is no immediate direct user interaction in this phase. The feature is infrastructure for future folding and indentation guide UX.

Observable effects today:
- None expected in normal editing behavior.
- Internal cache state becomes available for downstream features and tests.

## External Dependencies
No new third-party crates are required.

Uses existing project facilities:
- Buffer and syntax cache lifecycle.
- Configuration resolution for tab width.
- Existing test infrastructure.

## Error Handling
- If syntax rebuild fails or aborts, indent scope cache should not advance to a partial state.
- If tab width configuration cannot be read, normalization must safely fall back to width `4`.
- For malformed internal state during rebuild, fail the rebuild unit and keep the prior valid cache generation rather than panic in runtime editing paths.

## Security
No new external IO, process execution, or privileged behavior is introduced.

Input validation focus:
- Handle arbitrary line contents safely.
- Avoid integer overflow in visual-width calculations by using checked or saturating arithmetic where needed.

## Configuration
Indent normalization reads existing tab width configuration.

Rules:
- Preferred source: resolved editor tab width.
- Fallback: `4`.
- No new user-facing configuration options in this phase.

## Component Interactions
1. Edit mutates buffer lines.
2. Existing syntax invalidation marks syntax cache stale.
3. Invalidation also marks indent scope cache stale.
4. Rebuild computes syntax highlights and indent scopes from the same line snapshot.
5. Buffer stores new highlight cache and indent cache with aligned generation metadata.
6. Consumers read `scopes` and `line_to_scopes` through buffer accessors.

## Platform Considerations
Terminal platform differences do not affect scope computation because indent scopes are derived from buffer text and configuration, not terminal rendering behavior.
