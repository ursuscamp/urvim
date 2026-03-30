# Syntax Specializations Removal
## Summary
Remove the dedicated syntax specializations for block comments, keywords, types, and constants. Syntax highlighting should express those cases through standard delimiter and regex rules only, including explicit keyword rules and non-specific identifier rules for each language so identifiers still highlight appropriately. Line comments must remain supported as a first-class syntax field.

## Problem Statement
The syntax schema currently exposes special-case fields for several token categories that duplicate the general rule system. This creates two ways to express the same highlight behavior, increases maintenance cost, and makes syntax definitions less consistent. The editor only needs the line comment field to remain specialized for future comment-keybinding behavior.

## User Stories
- As a contributor, I want syntax definitions to use one consistent rule model, so that new grammars are easier to understand and maintain.
- As a user, I want existing line comment highlighting to continue working, so that the editor still recognizes comment prefixes for future comment actions.
- As a maintainer, I want block comments and identifier-based categories to be described with regular rules, so that syntax files stay simpler and closer to the underlying tokenizer.

## Functional Requirements
- [ ] **REQ-001**: Remove support for dedicated block comment fields from the syntax schema and loader.
- [ ] **REQ-002**: Remove support for dedicated keyword, type, and constant lists from the syntax schema and loader.
- [ ] **REQ-003**: Preserve support for the line comment field exactly as a top-level syntax property.
- [ ] **REQ-004**: Update builtin syntax definitions so block comments are expressed with delimiter rules rather than special fields.
- [ ] **REQ-005**: Update builtin syntax definitions so keyword, type, and constant highlighting is expressed with regex and delimiter rules rather than special lists.
- [ ] **REQ-006**: Ensure syntax loading rejects or ignores any removed special fields consistently, so syntax files cannot rely on them anymore.
- [ ] **REQ-007**: Add explicit keyword regex rules to each builtin syntax so known keywords continue to receive keyword highlighting through the general rule system.
- [ ] **REQ-008**: Add generic identifier rules to each builtin syntax so identifiers continue to receive language-appropriate highlighting through the general rule system.
- [ ] **REQ-009**: Keep existing syntax highlight output for supported filetypes functionally equivalent where possible, while using only the general rule system for the removed categories.
- [ ] **REQ-010**: Keep line comment highlighting behavior unchanged for all supported filetypes that currently use it.

## Non-Functional Requirements
- **Compatibility**: Existing themes and filetype detection should continue to load without requiring changes unrelated to the removed syntax fields.
- **Maintainability**: Syntax definitions should be easier to extend because common token categories are represented through the same rule primitives as other spans.
- **Reliability**: The loader should fail deterministically when it encounters now-unsupported syntax fields.

## Acceptance Criteria
- [ ] **AC-001**: Builtin syntax files no longer declare dedicated block comment, keyword, type, or constant sections.
- [ ] **AC-002**: The syntax schema no longer exposes block comment, keyword, type, or constant special-case fields.
- [ ] **AC-003**: Line comment highlighting still works for languages that define a line comment prefix.
- [ ] **AC-004**: Keyword, type, and constant highlighting still appears in builtins where equivalent regex or delimiter rules are added.
- [ ] **AC-005**: Each builtin language includes at least one keyword-focused regex rule that highlights known keywords according to that language's grammar.
- [ ] **AC-006**: Each builtin language includes at least one identifier-focused rule that highlights generic identifiers according to that language's grammar.
- [ ] **AC-007**: Syntax definitions that still use removed fields are rejected or normalized according to the new schema rules.

## Out of Scope
- Adding new commenting keybinds.
- Reworking the overall syntax rule engine beyond removing the specializations.
- Changing theme color names or removing unused theme syntax keys.
- Introducing new syntax categories beyond standard delimiter and regex rules.

## Assumptions
- The remaining generic rule types are sufficient to model the current builtin block comment and identifier highlighting behavior.
- The line comment field remains available because other editor features will depend on it later.
- Any theme entries for removed categories can remain temporarily even if syntax definitions stop emitting those categories.

## Dependencies
- Existing syntax schema and tokenizer implementation in `src/syntax.rs` and `src/buffer/syntax.rs`.
- Builtin syntax TOML files in `src/syntax_builtin/`.
- Syntax-related tests and fixtures that cover builtin highlighting behavior.
