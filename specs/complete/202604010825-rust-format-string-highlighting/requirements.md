# Context-Sensitive Highlighting Engine

## Summary
urvim should support context-sensitive syntax highlighting rules so that syntax definitions can change how text is tokenized based on surrounding structure. The first concrete use case is Rust formatting strings, where the engine should highlight formatting macro invocations so that the macro name, call punctuation, and format string content are styled distinctly, while only treating the string as a formatting string when it appears inside the formatting-call context.

## Problem Statement
The current syntax system is not expressive enough to model situations where the meaning of a token depends on surrounding structure. In particular, Rust formatting macros embed special interpolation syntax inside a string argument, but ordinary strings should not be treated the same way. The editor needs a general engine capability for context-sensitive highlighting so syntax definitions can apply different rules based on the active parse context, with Rust formatting strings serving as the first concrete implementation target.

## User Stories
- As a maintainer, I want the syntax engine to support context-sensitive highlighting rules, so that specialized cases can be modeled without turning every string or token into a special case.
- As a programmer, I want Rust formatting calls to highlight their name, call punctuation, and format string distinctly, so that I can recognize formatting contexts quickly.
- As a programmer, I want interpolation-like placeholders inside Rust format strings to be highlighted as part of the formatting language, so that I can read format arguments clearly.
- As a programmer, I want ordinary string literals to keep their existing highlighting when they are not part of a formatting context, so that non-format strings do not gain misleading structure.

## Functional Requirements
- [ ] **REQ-001**: The editor shall provide a context-sensitive highlighting mechanism that allows syntax definitions to change token classification based on surrounding syntax state.
- [ ] **REQ-002**: The context-sensitive highlighting mechanism shall support rules that depend on caller or container context, not only on the current token text.
- [ ] **REQ-003**: The editor shall allow syntax definitions to apply different rule sets to the same token form depending on the active context.
- [ ] **REQ-004**: The engine shall support a Rust formatting-string context as the first concrete rule set built on top of the context-sensitive mechanism.
- [ ] **REQ-005**: In Rust formatting contexts, the editor shall highlight the callee name, call punctuation, and the first string argument with distinct syntax styling.
- [ ] **REQ-006**: In Rust formatting contexts, the editor shall tag the callee name as `function` or `function.macro`.
- [ ] **REQ-007**: In Rust formatting contexts, the editor shall tag the call parentheses as `punctuation`.
- [ ] **REQ-008**: In Rust formatting contexts, the editor shall treat the first string argument as a formatting string with its own syntax rules.
- [ ] **REQ-009**: The editor shall not apply the Rust formatting-string rules to a string literal unless that string literal appears in the formatting-call context.
- [ ] **REQ-010**: Ordinary string literals outside formatting contexts shall continue to use the existing string highlighting behavior.
- [ ] **REQ-011**: The context-sensitive treatment shall not change buffer contents, cursor behavior, undo behavior, or save behavior.
- [ ] **REQ-012**: Any syntax definition that enables context-sensitive rules shall continue to validate and load successfully.

## Non-Functional Requirements
- **Compatibility**: The feature shall work with the current syntax highlighting and theme systems.
- **Reliability**: Highlighting shall remain correct under repeated edits within or around formatting-call invocations.
- **Maintainability**: Context-sensitive behavior shall stay scoped to syntax definitions or equivalent engine rules rather than becoming a global string rule.
- **Usability**: Context-sensitive constructs and format-string placeholders shall be visually distinguishable without additional user configuration.

## Acceptance Criteria
- [ ] **AC-001**: The syntax engine can apply different highlighting rules to the same token form based on surrounding context.
- [ ] **AC-002**: A Rust file containing a recognized formatting-call invocation highlights the callee name, parentheses, and format string with distinct syntax categories.
- [ ] **AC-003**: A Rust file containing a recognized formatting-call invocation with interpolation-like placeholder text highlights the placeholder region differently from the surrounding format string text.
- [ ] **AC-004**: A file containing a normal string literal such as `let s = "hello {name}";` keeps the existing non-format string highlighting and does not treat the string as a formatting string.
- [ ] **AC-005**: Editing text inside or around a context-sensitive construct updates the displayed highlighting without requiring a restart or manual refresh.
- [ ] **AC-006**: Syntax definitions that do not opt into context-sensitive rules continue to render string literals normally.

## Out of Scope
- Parser-backed highlighting engines such as tree-sitter.
- General expression parsing beyond what is needed to identify recognized context-sensitive constructs.
- Changing the meaning or runtime behavior of formatting calls or other syntax constructs.
- User-defined custom detection rules.
- Semantic highlighting from language servers.

## Assumptions
- The syntax system can represent a context-sensitive construct as a special-case region or equivalent scoped construct.
- The existing theme system already provides suitable style categories for function names, punctuation, strings, and nested placeholder regions.
- The first implementation can focus on syntax definitions that already have enough grammar information to identify the relevant Rust formatting-call forms.
- Recognizing context-sensitive constructs by name, surrounding syntax, or equivalent syntax context is sufficient for the initial release.

## Dependencies
- Existing syntax definition and rule-set support.
- Existing string-interpolation or nested-region support in the syntax engine.
- Existing theme syntax style categories.
- Existing regression test coverage for syntax highlighting.
