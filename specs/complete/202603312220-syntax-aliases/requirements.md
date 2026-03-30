# Syntax Aliases
## Summary
urvim should let syntax definitions declare one or more alias labels in metadata, and the syntax registry should resolve those alias labels to the same definition as the canonical syntax name. This makes common injected language labels such as Markdown fence tags work without requiring a one-to-one match on the canonical grammar name.

## Problem Statement
The current syntax registry only resolves canonical syntax names. That is good for internal identity, but it is too strict for user-provided injected language labels such as Markdown code fence tags, where authors often write short aliases like `js` or `ts` instead of the canonical names `javascript` or `typescript`. The built-in syntax set also lacks a declared alias list, so the editor cannot treat these common labels as first-class language identifiers.

## User Stories
- As a Markdown writer, I want `js` or `ts` fence tags to highlight correctly, so that I can use the labels people commonly write in documents.
- As a grammar maintainer, I want alias labels to live in syntax metadata, so that alias behavior is data-driven and easy to extend.
- As a contributor, I want alias collisions to fail at load time, so that two syntaxes can never claim the same injected language name.
- As a user, I want unknown injected language labels to keep falling back safely, so that unsupported aliases do not break highlighting.

## Functional Requirements
- [ ] **REQ-001**: Syntax metadata shall allow each syntax definition to declare zero or more alias labels in addition to its canonical name.
- [ ] **REQ-002**: The registry shall resolve a label to a syntax definition when the label matches either the canonical syntax name or one of its alias labels.
- [ ] **REQ-003**: Alias lookup shall be available to injected syntax resolution paths, including Markdown fenced code blocks and any other capture-based injected syntax selectors.
- [ ] **REQ-004**: Alias lookup shall not change filename or shebang syntax detection behavior unless the resolved label is explicitly provided through a label-based lookup path.
- [ ] **REQ-005**: Built-in syntax definitions shall declare well-known alias labels for every supported non-fallback language syntax.
- [ ] **REQ-006**: The loader shall reject an alias that is empty after trimming.
- [ ] **REQ-007**: The loader shall reject duplicate alias labels within a single syntax definition.
- [ ] **REQ-008**: The loader shall reject any alias label that duplicates another syntax's canonical name or alias label.
- [ ] **REQ-009**: Alias matching shall be normalized consistently so that equivalent injected labels resolve deterministically.
- [ ] **REQ-010**: Existing canonical-name lookups shall continue to work unchanged.
- [ ] **REQ-011**: Unknown or unresolved injected labels shall continue to use the region's configured fallback behavior.

## Non-Functional Requirements
- **Reliability**: Alias collisions and malformed aliases shall fail during syntax load rather than producing ambiguous runtime behavior.
- **Compatibility**: Existing canonical syntax names shall remain stable and continue to resolve exactly as before.
- **Usability**: Common injected labels shall work without requiring document authors to know the repository's canonical syntax names.
- **Maintainability**: Alias data shall remain declarative in syntax metadata rather than being hardcoded in lookup logic.

## Acceptance Criteria
- [ ] **AC-001**: A Markdown fence tagged `js` resolves to the JavaScript syntax definition.
- [ ] **AC-002**: A Markdown fence tagged `ts` resolves to the TypeScript syntax definition.
- [ ] **AC-003**: A Markdown fence tagged with an unsupported label still falls back according to the region's configured fallback policy.
- [ ] **AC-004**: Two syntax definitions cannot load successfully if they both claim the same alias label.
- [ ] **AC-005**: A syntax definition can declare alias labels without changing its canonical name or display name.
- [ ] **AC-006**: All supported built-in non-fallback syntaxes load with at least one declared alias label.
- [ ] **AC-007**: Canonical-name resolution continues to succeed for existing internal lookups and tests.

## Out of Scope
- Introducing per-user alias configuration at runtime.
- Changing filename extension matching rules.
- Renaming canonical syntax definitions.
- Runtime hot-reloading of syntax metadata.

## Assumptions
- Alias matching is intended for user-provided labels and injected syntax selectors, not for changing how file paths are classified.
- The supported built-in syntax set is the authoritative source for the initial alias table.
- Alias normalization should be deterministic and simple enough to keep Markdown fence resolution predictable.

## Dependencies
- Existing syntax registry and syntax loader.
- Existing injected syntax resolution path.
- Built-in syntax definition files under `src/syntax_builtin/`.
- Regression tests for syntax resolution and Markdown fence highlighting.
