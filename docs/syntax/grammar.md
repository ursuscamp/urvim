# Syntax Grammar Files

This document describes urvim's TOML-based syntax grammar format.

Grammar files are data. A syntax definition is made of metadata plus one ordered `rules` list.
Rules are evaluated from top to bottom, and the first match at the current scan position wins.

Tag names are described in [docs/syntax/tags.md](docs/syntax/tags.md).

## File Layout

```toml
[metadata]
name = "example"
display_name = "Example"
alias = ["ex"]
glyph = ""
glyph_color = "#dea584"
filename = ["\\.ex$"]
shebang = ["^#!.*\\bexample(?:\\s|$)"]
comment_prefix = "#"

[[rules]]
kind = "regex"
pattern = "#.*$"
tag = "comment"
```

The `[metadata]` section tells urvim how to recognize and present the syntax.
The `rules` list describes how to color text once the syntax has been selected.

urvim parses builtin grammar files into a raw registry first and promotes a syntax to its compiled form only when it is actually needed for tokenization.

## Metadata

| Field | Purpose | Guidance |
|---|---|---|
| `name` | Canonical syntax name | Use a short, stable, lower-case identifier such as `javascript` or `markdown`. |
| `display_name` | Human-readable label | Use the name you want in the status bar and any UI labels. |
| `alias` | Alternate labels | Add common shorthand labels such as `js`, `md`, or `rb`. |
| `comment_prefix` | Canonical line comment prefix | Use the token that should be inserted and removed by line-comment toggle actions, such as `//`, `#`, or `--`. Leave it unset for syntaxes without a line comment form. |
| `glyph` | Optional filetype icon | Use a nerdfont glyph when the language should appear with an icon in the tab bar and status bar. Leave it unset for languages without a good icon. |
| `glyph_color` | Optional glyph foreground color | Use a default foreground color associated with the language icon. The value can be an ANSI index or an RGB hex color, matching the editor's other color literals. |
| `filename` | Filename-matching regexes | Use these for extensions or fixed names. The loader matches against the lower-cased basename, so write the pattern as if the filename were lower-case. |
| `shebang` | Shebang regexes | Use these for shebangs, magic comments, and other header markers. These are checked before filename matching. |

## Rules

Rules are ordered. Keep specific rules before broad fallback rules.

The supported rule kinds are:

- `regex`
- `injection`

### `regex`

Use `kind = "regex"` for single-pattern matches that do not need a closing delimiter.

```toml
[[rules]]
kind = "regex"
pattern = "//.*$"
tag = "comment.line"
```

Best uses:

| Good fit | Why |
|---|---|
| comments | They are usually simple and line-oriented. |
| keywords | A word boundary regex is often enough. |
| numbers, constants, simple identifiers | These are easy to describe with one pattern. |
| language directives | Preprocessor lines, annotations, and decorators often fit well here. |

### `injection`

Use `kind = "injection"` when the current context should delegate body highlighting to another syntax.

```toml
[[rules]]
kind = "injection"
selector = { name = "javascript" }
fallback = "unstyled"
context = { requires = ["script_host"] }
```

Use `selector = { capture = "..." }` when the opener text chooses the nested syntax.

## Context Markers

Context markers let rules depend on earlier matches. They can be plain markers or payload-bearing entries that carry opener-specific text forward.

Payloads are useful when a later rule needs more than just “this mode is active.” For example, heredoc-style rules can push a marker with the captured terminator text, then later require that the active payload prefix-matches the closing token before the context is popped.

The payload-bearing form looks like this:

```toml
[[rules]]
kind = "regex"
pattern = "<<(EOF|END)"
tag = "string"
context = { push = [{ name = "heredoc", capture = 1 }, "heredoc_body"] }

[[rules]]
kind = "regex"
pattern = "EOF|END"
tag = "string"
context = { requires = ["heredoc"], payload_match = { name = "heredoc" }, pop = ["heredoc", "heredoc_body"] }
```

In this example, the opener captures the terminator text into the `heredoc` context entry, and the closer only matches when the current text is compatible with that stored payload.

```toml
[[rules]]
kind = "regex"
pattern = "\""
tag = "string"
context = { push = ["in_string"] }

[[rules]]
kind = "regex"
pattern = "\""
tag = "string"
context = { requires = ["in_string"], pop = ["in_string"] }
```

`requires` checks for presence anywhere in the active context stack.
`push` adds markers or payload-bearing entries, and `pop` removes the most recent matching entry.

## Choosing The Right Shape

| Shape | Use when |
|---|---|
| `regex` | The match is local and self-contained. |
| `injection` | The active context should delegate the body to another syntax. |

## Validation

The loader validates:

- metadata names and aliases
- comment prefixes
- glyphs and glyph colors
- tags
- regexes
- context markers
- injected syntax targets

## Small Complete Example

```toml
[metadata]
name = "example"
display_name = "Example"
alias = ["ex"]
glyph = ""
glyph_color = "#dea584"
filename = ["\\.ex$"]
shebang = []
comment_prefix = "#"

[[rules]]
kind = "regex"
pattern = "#.*$"
tag = "comment"

[[rules]]
kind = "injection"
selector = { name = "javascript" }
fallback = "unstyled"
context = { requires = ["script_host"] }
```

If you are writing a new grammar, start with metadata, then add a small ordered `rules` list, and only add context or injection when the syntax genuinely needs it.
