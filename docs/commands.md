# Commands

## Syntax

- Commands are whitespace-delimited.
- Quoted strings are supported.
- Named arguments use `arg=value`.
- Positional arguments are allowed when the command is unambiguous.
- Top-level aliases remain supported for common commands.
- `action` is the canonical namespace for editor actions; `cursor`, `mode`, `operator`, and `surround` are also accepted directly.

## Command Shape

- Use nested subcommands to choose an operation.
- Use arguments to provide data for that operation.
- Prefer a named argument when the value is user data like a path, count, character, filetype, target, or delimiter.
- Allow positional shorthand only when there is one obvious value.

Examples:

- `action cursor left count=3`
- `action cursor find-forward char=x`
- `buffer write path=notes.txt`
- `lsp rename name=new_name`

In contrast, `left`, `find-forward`, `write`, and `rename` are operation selectors, not arguments.

## Aliases

Aliases are registered commands that expand to canonical command prefixes before
resolution. They are the first use of the custom command registry; future plugin
commands should register through the same system.

- `write` -> `buffer write`
- `write-all` -> `buffer write-all`
- `edit` -> `buffer edit`
- `quit` -> `app quit`
- `try-quit` -> `app try-quit`
- `command-line` -> `app command-line`
- `completion` -> `app completion`
- `mode` -> `action mode`
- `cursor` -> `action cursor`
- `operator` -> `action operator`
- `surround` -> `action surround`

## Shared Arguments

- Arguments marked with `*` can also be provided positionally.
- `count=<positive-integer>` repeats countable actions. Countable commands also accept positional shorthand, for example `action cursor left 3`.
- `register=<character>` applies the action to a register when the action supports registers.
- `char=<character>` accepts exactly one character. Find and till commands also accept positional shorthand, for example `action cursor find-forward x`.

## Buffer

- `buffer write`
- `buffer write path=<path>*`
- `buffer write-all`
- `buffer write_all`
- `buffer writeall`
- `buffer edit`
- `buffer edit path=<path>*`
- `buffer filetype filetype=<filetype>*`
- `buffer set-filetype filetype=<filetype>*`

## Action Mode

- `action mode normal`
- `action mode insert`
- `action mode replace`
- `action mode visual`
- `action mode visual-line`
- `action mode resizing`

## Action Cursor

These commands accept `count=<positive-integer>` and `register=<character>` unless otherwise rejected by the action at runtime.

- `action cursor left`
- `action cursor right`
- `action cursor up`
- `action cursor down`
- `action cursor page-up`
- `action cursor page-down`
- `action cursor half-page-up`
- `action cursor half-page-down`
- `action cursor line-start`
- `action cursor line-end`
- `action cursor line-content-start`
- `action cursor file-start`
- `action cursor file-end`
- `action cursor screen-top`
- `action cursor screen-middle`
- `action cursor screen-bottom`
- `action cursor paragraph-previous`
- `action cursor paragraph-next`
- `action cursor diff-previous`
- `action cursor diff-next`
- `action cursor diff-end-previous`
- `action cursor diff-end-next`
- `action cursor match-bracket`
- `action cursor find-forward char=<character>*`
- `action cursor find-backward char=<character>*`
- `action cursor till-forward char=<character>*`
- `action cursor till-backward char=<character>*`
- `action cursor repeat-find`
- `action cursor repeat-find-reverse`

## Action Edit

These commands accept `count=<positive-integer>` and `register=<character>` unless otherwise rejected by the action at runtime. `undo`, `redo`, and `repeat-last-change` do not accept positional counts.

- `action edit delete-forward`
- `action edit delete-backward`
- `action edit delete-selection`
- `action edit delete-line`
- `action edit yank-line`
- `action edit yank-selection`
- `action edit change-line`
- `action edit change-selection`
- `action edit change-to-line-end`
- `action edit paste-after`
- `action edit paste-before`
- `action edit join-space`
- `action edit join-no-space`
- `action edit indent-decrease`
- `action edit indent-increase`
- `action edit toggle-line-comment`
- `action edit undo`
- `action edit redo`
- `action edit repeat-last-change`
- `action edit append-after-cursor`
- `action edit append-to-line-end`
- `action edit insert-at-line-start`
- `action edit open-line-below`
- `action edit open-line-above`

## Action Operator

Operators require `target=<operator-target>`. They also accept `count=<positive-integer>` and `register=<character>`.

- `action operator delete target=<operator-target>*`
- `action operator change target=<operator-target>*`
- `action operator yank target=<operator-target>*`
- `action operator lowercase target=<operator-target>*`
- `action operator uppercase target=<operator-target>*`
- `action operator toggle-case target=<operator-target>*`

Operator targets:

- `selection`
- `word`
- `word-forward`
- `word-end`
- `word-backward`
- `big-word`
- `big-word-forward`
- `big-word-end`
- `big-word-backward`
- `line-start`
- `line-end`
- `line-content-start`
- `first-line`
- `last-line`
- `inner-word`
- `around-word`
- `inner-big-word`
- `around-big-word`
- `inner-paren`
- `around-paren`
- `inner-square`
- `around-square`
- `inner-curly`
- `around-curly`
- `inner-angle`
- `around-angle`

## Action Tab

- `action tab previous`
- `action tab next`

## Action Jump

- `action jump backward`
- `action jump forward`

## Action Surround

- `action surround add target=<text-object>* delimiter=<delimiter-family>*`
- `action surround delete target=<delimiter-family>*`
- `action surround replace target=<delimiter-family>* replacement=<delimiter-family>*`

Text objects:

- `word`
- `inner-word`
- `big-word`
- `inner-big-word`
- `paren`
- `inner-paren`
- `square`
- `inner-square`
- `curly`
- `inner-curly`
- `angle`
- `inner-angle`
- `double-quote`
- `inner-double-quote`
- `single-quote`
- `inner-single-quote`
- `backtick`
- `inner-backtick`

Delimiter families:

- `(`
- `)`
- `paren`
- `[`
- `]`
- `square`
- `{`
- `}`
- `curly`
- `<`
- `>`
- `angle`
- `<LessThan>`
- `<GreaterThan>`
- `"`
- `double-quote`
- `'`
- `single-quote`
- `` ` ``
- `backtick`

## Pick

- `pick file`
- `pick buffer`
- `pick git`
- `pick grep`
- `pick colorscheme`
- `pick filetype`
- `pick doc-symbols`
- `pick document-symbols`
- `pick workspace-symbols`
- `pick references`
- `pick code-actions`

## Lsp

- `lsp hover`
- `lsp definition`
- `lsp references`
- `lsp rename`
- `lsp rename name=<new-name>`
- `lsp code-actions`
- `lsp diagnostic previous`
- `lsp diagnostic next`
- `lsp diagnostic previous-error`
- `lsp diagnostic next-error`
- `lsp diagnostic error-previous`
- `lsp diagnostic error-next`

## Pane

- `pane split-vertical`
- `pane split-horizontal`
- `pane focus-left`
- `pane focus-right`
- `pane focus-up`
- `pane focus-down`
- `pane resize-left count=<positive-integer>`
- `pane resize-right count=<positive-integer>`
- `pane resize-up count=<positive-integer>`
- `pane resize-down count=<positive-integer>`
- `pane equalize`
- `pane wrap-toggle`
- `pane close`

## App

- `app command-line`
- `app completion`
- `app try-quit`
- `app quit`

## Quoted Paths

- `edit "notes/today file.txt"`
- `write "output/new name.txt"`
