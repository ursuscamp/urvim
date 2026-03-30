# Lazy Syntax Promotion - Technical Design

## Architecture Overview
The syntax subsystem will move from eager builtin compilation to lazy promotion.

At startup, the editor will still parse every builtin syntax TOML file, validate its raw shape, and index its metadata. It will not compile every rule set into runtime regex/state objects up front. Instead, the registry will keep each builtin syntax in a raw form until a caller asks for that syntax by canonical name, alias, filename match, shebang match, or injected/nested resolution.

Promotion is synchronous and on-demand. The first lookup for a syntax compiles it, caches the compiled result, and returns the promoted definition. Subsequent lookups reuse the cached compiled definition.

## Interface Design
### Registry lookup
The registry will expose promotion-aware lookup methods that return a compiled syntax definition for use by buffer classification and tokenization.

Conceptually:

```rust
pub fn resolve_for_input(
    &self,
    path: Option<&Path>,
    shebang: Option<&str>,
) -> Option<Arc<SyntaxDefinition>>;

pub fn resolve_label(&self, label: &str) -> Option<Arc<SyntaxDefinition>>;
pub fn promote(&self, name: &str) -> Result<Arc<SyntaxDefinition>, SyntaxLoadError>;
```

### Top-level buffer classification
Buffer creation and refresh paths will use metadata lookup to find the best matching syntax, then request promotion only for that selected syntax. This preserves correct filetype classification without compiling the entire builtin catalog.

### Injected and nested resolution
Tokenizer code that resolves an injected syntax selector will call the same promotion-aware registry path. If the selector identifies a syntax that has not yet been compiled, the registry will promote it at that moment and the tokenizer will store the compiled result for reuse while scanning the nested region.

## Data Models
### Syntax registry entry
Each builtin syntax will be represented by an entry that keeps raw source data and an optional compiled payload.

```rust
enum SyntaxEntry {
    Raw(RawSyntaxDocument),
    Compiled(Arc<SyntaxDefinition>),
}
```

### Registry indexes
The registry will continue to maintain metadata indexes for:
- canonical syntax names
- aliases
- filename patterns
- shebang patterns

These indexes will point to canonical names rather than requiring every syntax to be compiled during startup.

### Compiled syntax ownership
Compiled syntax definitions will be stored behind shared ownership so that the same promoted definition can be reused by buffer classification and nested syntax tokenization without copying the full compiled structure.

## Key Components
### SyntaxRegistry
Responsibilities:
- parse builtin TOML sources into raw entries
- validate raw syntax structure
- answer metadata lookups without promotion
- promote raw syntaxes to compiled syntax definitions on demand
- cache promoted syntaxes for later reuse

Dependencies:
- builtin syntax sources from `src/syntax/builtins`
- syntax parsing and compilation helpers already present in `src/syntax/mod.rs`

### Buffer syntax selection
Responsibilities:
- determine the buffer's syntax name from file path or shebang hints
- request promotion only for the selected top-level syntax
- keep the buffer's syntax label behavior unchanged

### Nested syntax tokenizer
Responsibilities:
- resolve injected syntax selectors when a nested region is encountered
- request promotion for the nested target syntax the first time it is needed
- retain the promoted syntax for the rest of the nested region state

## User Interaction
No new user-facing configuration or commands are required.

The visible effects are:
- faster editor startup
- the same syntax labels and highlighting results as before
- a possible one-time delay the first time a syntax is encountered

## External Dependencies
No new external crates are required.

The implementation can use the existing `regex`, `toml`, `serde`, and `tracing` dependencies already present in the project.

## Error Handling
Promotion uses the same validation and error reporting as the current eager path.

Expected cases:
- malformed builtin TOML still fails during startup parsing
- structurally valid but semantically invalid syntax data fails when that syntax is first promoted
- unresolved aliases, filenames, or shebang matches fall back to `plaintext`
- unresolved injected syntax selectors continue to follow the existing fallback behavior already encoded in the tokenizer

If promotion fails for a syntax that was selected for actual use, the error should surface deterministically instead of partially registering a corrupt definition.

## Security
This change does not introduce new security-sensitive behavior.

The registry only loads embedded builtin syntax sources and does not expand its trust boundary. Promotion is a local in-memory operation and does not involve untrusted network or filesystem inputs beyond existing file-open paths used for filetype detection.

## Configuration
No new configuration options are required.

The existing syntax enable/disable behavior and current startup config remain unchanged.

## Component Interactions
1. Startup loads the builtin syntax catalog into raw registry entries and metadata indexes.
2. A buffer is created or refreshed.
3. The buffer asks the registry for the best matching top-level syntax using path and/or shebang metadata.
4. The registry promotes only that chosen syntax if it is still raw.
5. The buffer stores the promoted syntax name for labeling and cache keying.
6. While tokenizing a line, the syntax engine encounters an injected region.
7. The injected selector resolves a nested syntax label.
8. The registry promotes that injected syntax on demand if needed.
9. The tokenizer stores the promoted nested syntax in region state and reuses it for the rest of the region.

## Platform Considerations
The design is platform-agnostic.

The only observable platform differences should remain the existing ones around file path detection and terminal behavior; lazy syntax promotion does not depend on OS-specific APIs.
