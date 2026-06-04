# Syntax Tags

This document describes the standard syntax tag vocabulary used by urvim's syntax system.
Tokenizer authors may define additional valid tags, but the tags below are the recommended base set for common syntax concepts.

Theme authors should use these tags under the `syntax.` highlight namespace when styling them in a theme file.

## Resolution Rules

- Tags are hierarchical and dot-separated.
- Themes should try the most specific tag first.
- If an exact tag is not defined, theme lookup should fall back to the nearest parent tag.
- If no parent tag is defined, the theme default style applies.
- Theme highlight names use the same parent fallback rules whether they live under `ui.` or `syntax.`.

## Canonical Top-Level Tags

- `comment`
- `constant`
- `function`
- `function.macro`
- `keyword`
- `namespace`
- `markup`
- `operator`
- `punctuation`
- `string`
- `type`
- `variable`

## Recommended Child Tags

- `comment.block`
- `comment.documentation`
- `comment.line`
- `comment.todo`
- `comment.fixme`
- `comment.bug`
- `comment.note`
- `constant.boolean`
- `constant.float`
- `constant.integer`
- `constant.null`
- `constant.number`
- `function.method`
- `namespace.module`
- `markup.code`
- `markup.code.inline`
- `markup.emphasis`
- `markup.heading`
- `markup.link`
- `markup.list`
- `markup.quote`
- `markup.strong`
- `markup.thematic_break`
- `string.escape`
- `string.interpolation`
- `type.parameter`
- `variable.global`
- `variable.parameter`
- `variable.property`

## String Interpolation

`string.interpolation` is the recommended child tag for embedded expression-like
regions inside strings, including placeholder-style bodies in language-specific
format strings. Tokenizers may still use other existing tags such as `punctuation`,
`variable`, or `number` for the pieces within that region when those tags better
describe the syntax being highlighted.

## Markup Body Convention

For markup-style tokenizers, it is a good practice to tag the interior body text of a markup region with a `.text` refinement when that body should be styleable independently of surrounding punctuation or delimiters.

Examples:

- `markup.quote.text`
- `markup.list.text`
- `markup.heading.text`
- `markup.code.inline.text`

The `.text` refinement is a convention, not a requirement. Tokenizers may use it when it improves styling control.
