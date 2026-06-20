# Builtin Syntax Tokenizers

urvim no longer uses TOML syntax grammar files. Builtin syntaxes are defined in Rust metadata and highlighted by Rust tokenizer modules.

Tag names are described in [tags.md](tags.md).

## File Layout

| File                                        | Purpose                                                    |
| ------------------------------------------- | ---------------------------------------------------------- |
| `src/syntax/builtin.rs`                     | Builtin syntax metadata catalog.                           |
| `src/syntax/definition.rs`                  | `SyntaxDefinition`, metadata types, and `SyntaxTokenizer`. |
| `src/syntax/registry.rs`                    | Registry construction and syntax resolution.               |
| `src/syntax/builtin_tokenizers/mod.rs`      | Tokenizer module declarations and dispatch.                |
| `src/syntax/builtin_tokenizers/<name>.rs`   | One line tokenizer for a syntax.                           |
| `src/buffer/tests/syntax/<name>.rs`         | Syntax regression tests.                                   |
| `crates/urvim_syntax/fixtures/<name>.<ext>` | Fixture text used by syntax tests.                         |

## Metadata

Add or update metadata in `src/syntax/builtin.rs`.

Important fields:

| Field            | Purpose                                                    |
| ---------------- | ---------------------------------------------------------- |
| `name`           | Canonical syntax name.                                     |
| `display_name`   | Label shown in UI.                                         |
| `alias`          | Alternate names.                                           |
| `filename`       | Filename regexes matched against the lower-cased basename. |
| `shebang`        | Shebang/header regexes checked before filename matching.   |
| `comment_prefix` | Line comment prefix used by comment toggling.              |
| `tokenizer`      | `SyntaxTokenizer` variant dispatched for highlighting.     |

Filename and shebang metadata still use regexes. Runtime tokenization should live in builtin scanner code rather than syntax grammar files.

## Tokenizer Shape

Each tokenizer exports a function like:

```rust
pub(crate) fn tokenize_example_line(
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    // scan line and return spans plus updated state
}
```

The usual structure is:

1. Restore `ContextStack`, injection state, and parent style from `SyntaxState`.
2. Walk the line with a byte index.
3. Handle active multiline contexts first, such as block comments, strings, heredocs, or injections.
4. Match top-level comments, strings, numbers, keywords, identifiers, punctuation, and operators.
5. Push `SyntaxSpan::new(start, end, tag)` for styled regions.
6. Return `SyntaxState::Code(CodeState::RuleList { ... })` with updated context.

## State

Use `ContextStack` for anything that can continue past the current line.

Common patterns:

- `ctx.push("name")` when an opener is found.
- `ctx.pop_top("name")` when a closer must match the top of the stack.
- `ctx.top_is("name")` to route scanning while a context is active.
- `ctx.contains_anywhere("name")` only when non-top membership is intentional.
- `ctx.push_with_payload("name", payload)` for heredoc/code-fence terminators.
- `ctx.payload_for("name")` to read that payload later.

Keep closing-rule checks before opening-rule checks when both can match at the same byte. That preserves expected closing preference for nested or injected contexts.

## Tokenizer Contract

Builtin scanners are the canonical syntax implementation.

1. Tokenizers scan one line at a time and return updated cross-line state.
2. State must stay deterministic, cloneable, and comparable so syntax caches can reconverge after edits.
3. Strictly nested contexts should use stack-top checks like `top_is` and `pop_top`.
4. Broad host flags should use `contains_anywhere` only when non-top membership is intentional.
5. Closing rules should run before opening rules when both match the same byte.
6. Injection boundary scans must check every byte position for any host rule that can end the injected body.
7. Direct byte-level checks are preferred in hot paths.
8. Shared helpers should only be used when semantics are identical; language-specific quirks should stay local and explicit.
9. Public syntax modules, types, and methods need documentation comments.
10. New or changed tokenizer behavior should include fixture coverage and exact-span tests where precedence or boundaries matter.

See also [docs/syntax/highlighting.md](highlighting.md) for the broader syntax pipeline.

## Tags

Define frequently used tags as `static LazyLock<Tag>` values near the top of the tokenizer.

```rust
tag_static!(COMMENT, "comment");
tag_static!(KW, "keyword");
tag_static!(S, "string");
```

Tokenizers should use semantic tags such as `keyword`, `string`, `comment.block`, `number`, `variable.property`, and `markup.tag`. Theme lookup adds the `syntax.` namespace later.

## Performance Guidelines

- Prefer direct byte-level checks such as `tail.starts_with(...)`, `as_bytes()`, and `char_indices()`.
- Avoid `format!()` and other allocation-heavy work in hot scanning paths.
- Consume runs of text in one span when possible instead of advancing one byte at a time.
- Keep helper functions small and syntax-local unless sharing avoids meaningful duplication.
- Precompute parsed `Tag` values with `LazyLock`.

## Tests

When adding or changing a tokenizer:

1. Update or add a fixture in `crates/urvim_syntax/fixtures/`.
2. Add exact-span tests in `src/buffer/tests/syntax/<name>.rs` for bug-prone forms.
3. Run the focused test, for example `cargo test buffer::tests::syntax::ruby`.
4. Run `cargo check`.

Useful assertions include:

- `assert_spans_include_style(...)` for broad smoke coverage.
- `assert_spans_include_exact_style(...)` for regressions where a specific token must have a specific tag.

## Debugging

Use `dump_tokens` to inspect actual spans:

```sh
cargo run --bin dump_tokens -- path/to/file
```

The output is JSON-lines with byte ranges, styles, and token text. It is the fastest way to diagnose surprising highlighting.
