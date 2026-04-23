# Syntax Highlighting Tutorial

This document explains how urvim's syntax highlighting works from end to end.

urvim does not parse source code into an AST. Instead, it chooses a syntax definition, walks the buffer line by line, produces tagged spans, maps tags to theme styles, and renders the result to the terminal.

## Main Files

| File | Role |
|---|---|
| `src/syntax/mod.rs` | Loads and validates syntax grammar files, builds the registry, resolves syntax names, and promotes compiled syntax definitions on demand. |
| `src/syntax/builtins/*.toml` | Built-in syntax grammar files. |
| `src/buffer/io.rs` | Chooses an initial syntax when a buffer is created from text or a file path. |
| `src/buffer/mod.rs` | Stores the active syntax name, resolves display labels, and refreshes syntax when the buffer changes. |
| `src/buffer/syntax.rs` | Tokenizes lines, caches syntax state, and computes highlight spans. |
| `docs/background-jobs.md` | Describes the internal deferred-work framework that syntax catch-up uses. |
| `src/window/view.rs` | Requests spans for visible lines and converts tags into highlight overlays. |
| `src/theme/model.rs` | Defines theme style data, including the unified highlight-name mapping. |
| `src/window/render.rs` | Applies the chosen line base style and writes styled chunks to the terminal screen. |

## Core Concepts

### Syntax definition

A syntax definition contains:

- metadata such as `name`, `display_name`, `alias`, `filename`, `shebang`, and `comment_prefix`
- one ordered `rules` list

### Rule

Rules are matched in order. The supported rule kinds are:

- `regex`
- `injection`

### Tag

A tag is the semantic label attached to a match, such as `keyword`, `string`, or `comment.line`.

### Syntax cache

The buffer keeps a cache of syntax state and spans line by line, so edits only invalidate the necessary suffix of the buffer.

When a buffer is rendered, urvim uses any cached spans it already has for the visible viewport. If the cache has not reached a line yet, the editor paints that line with the base theme style first and lets background syntax catch-up fill in the missing spans later.
Background syntax catch-up is submitted in latest-only mode, so rapid edits can cancel older queued highlight work before it runs. The editor keeps showing the last completed syntax state while fresher work is still pending.

## High-Level Flow

```mermaid
flowchart TD
    A["Buffer is created or loaded"] --> B["Resolve syntax from path or shebang"]
    B --> C["Store canonical syntax name in Buffer"]
    C --> D["Render requests visible lines"]
    D --> E["Use cached spans when they already exist"]
    E --> F["Render visible lines immediately"]
    F --> G["Background catch-up fills in missing cache lines"]
    G --> H["Tokenizer walks lines using ordered rules"]
    H --> I["Spans are returned as tags"]
    I --> J["Theme maps tags to styles"]
    J --> K["Terminal renders styled chunks"]
    L["Text edit"] --> M["Invalidate syntax from changed line"]
    M --> E
```

## Choosing A Syntax

Syntax selection happens before highlighting begins.

The key entry point is `src/syntax/mod.rs`, where `resolve_builtin_syntax` checks:

1. `shebang` patterns
2. filename patterns
3. the fallback syntax, which is `plaintext`

The syntax resolution code also handles aliases.

## Tokenization

`Buffer::syntax_spans_for_line()` in `src/buffer/syntax.rs` asks the syntax cache to compute the requested line.

The cache makes sure earlier lines have already been tokenized, then returns the cached spans for the requested line.

If the requested line is not cached yet, the render path does not block waiting for the full file. It uses the current base style for that line and relies on the background job framework to catch up afterward.
That background work may be superseded by newer edits, but the last completed highlight stays visible until a fresher result is accepted.

The tokenizer walks the line from left to right and tries the active rules in order.
The first matching rule wins, so specific patterns should come before broad fallback patterns.

## Syntax State

urvim keeps state across lines so multiline strings, block comments, code fences, and injected bodies can continue correctly.

Context markers are the main way rules communicate with later rules:

- `requires` checks whether a marker is already active
- `push` adds markers or payload-bearing entries
- `pop` removes the most recent matching entry

## Rendering

The tokenizer returns spans tagged with semantic labels like `keyword`, `string`, or `markup.code`.

`src/window/view.rs` translates those tags into highlight overlays, then `src/window/render.rs` applies the chosen line base style and writes the final styled chunks to the terminal.

After the syntax spans are available, urvim can layer comment-scoped todo highlighting on top of them during rendering. That overlay scans only comment spans, looks for configured standalone markers such as `TODO` and `FIXME`, and applies marker-specific tags like `comment.todo` without changing the underlying buffer text.

Theme highlights use the unified hierarchical naming model:

- UI chrome uses `ui.*` names such as `ui.status_bar` and `ui.window.active_line`
- gutter row emphasis uses `ui.window.gutter.active_line`
- syntax styling uses `syntax.*` names such as `syntax.comment` and `syntax.string.interpolation`
- the lookup rules are hierarchical, so the nearest defined parent wins when a specific highlight is missing
- highlight lookup returns the explicit overlay for that name; renderers decide which base style to layer underneath it

When a syntax tag is resolved, urvim maps the raw tag into the syntax highlight namespace before asking the theme for an overlay. That keeps the grammar vocabulary and the theme vocabulary aligned without requiring the grammar tags themselves to be renamed.

Renderers then choose the base style explicitly. A window line uses the theme default style on ordinary lines and `theme.default_style().overlay(ui.window.active_line)` on the active line before chunk overlays are applied.

So the pipeline is:

`grammar -> tags -> unified theme overlays -> renderer-chosen base style -> terminal output`

## After An Edit

Text edits live in `src/buffer/edit.rs`.
Every mutation invalidates syntax from the first changed line onward.

That matters because syntax state can spill across lines.
If line 10 changes, urvim cannot safely trust syntax results from line 10 onward, so it recomputes from that point forward using the preserved state from earlier lines.
Any background catch-up result carries the buffer generation it was computed against, and stale results are discarded if the buffer changed in the meantime.
Older queued catch-up jobs for the same buffer are also pruned before execution, which keeps the worker focused on the newest visible syntax state.

## Practical Advice

- Put comments and other structural matches before broad fallback matches.
- Keep regexes narrow.
- Use `lookahead` for call-style identifiers when you want the name highlighted
  only when a delimiter like `(` follows immediately after optional whitespace.
- Use `namespace` for qualified prefixes and module paths like `std::`,
  `crate::`, or `super::`.
- Use `context` when a rule only makes sense after an earlier opener.
- Reach for `injection` when a body needs nested highlighting.

## Example

```toml
[metadata]
name = "example"
display_name = "Example"
filename = ["\\.ex$"]
shebang = []

[[rules]]
kind = "regex"
pattern = "#.*$"
tag = "comment"

[[rules]]
kind = "injection"
selector = { capture = "^[ \\t]*([A-Za-z0-9_+-]+)" }
fallback = "unstyled"
context = { requires = ["script_host"] }
```
