# Lazy Syntax Promotion

## Summary
Reduce editor startup time by parsing builtin syntax sources up front while deferring syntax compilation until a syntax is actually needed. Top-level syntax selection should continue to work when a file is opened, but injected and nested syntaxes should be compiled only when tokenization actually encounters them.

## Problem Statement
The editor currently compiles all builtin syntaxes during startup. That work is measurable and noticeable, with startup logging showing roughly 660 ms spent loading 41 builtin syntax files before the editor is fully interactive. This creates avoidable lag even when the user only needs one top-level syntax, and it also means injected syntax definitions are compiled long before they are ever encountered.

## User Stories
- As a user, I want the editor to start quickly, so that launch feels responsive.
- As a user, I want opening a file to still classify its syntax correctly, so that highlighting and labels continue to work.
- As a user, I want nested and injected syntaxes to continue highlighting correctly, so that Markdown fences and similar constructs behave as expected.
- As a developer, I want injected syntax compilation to happen only when a nested region actually needs it, so that syntax work is deferred until tokenization reaches that region.

## Functional Requirements
- [ ] **REQ-001**: The syntax registry shall parse builtin syntax source files without compiling every syntax definition during startup.
- [ ] **REQ-002**: The syntax registry shall preserve enough metadata from uncompiled syntaxes to resolve names, aliases, filename matches, and shebang matches.
- [ ] **REQ-003**: The syntax registry shall continue to resolve the correct top-level syntax for a buffer opened from a file path or shebang hint without requiring all builtin syntaxes to be compiled first.
- [ ] **REQ-004**: The tokenizer shall promote an injected or nested syntax definition the first time that region is encountered during highlighting.
- [ ] **REQ-005**: Once a syntax has been promoted, subsequent lookups shall reuse the compiled definition rather than recompiling it.
- [ ] **REQ-006**: Launching the editor with an untitled buffer shall not require compiling the entire builtin syntax catalog.
- [ ] **REQ-007**: If a builtin syntax cannot be promoted because its definition is invalid, the editor shall fail deterministically using the existing syntax-load error behavior.
- [ ] **REQ-008**: The visible syntax label and highlighting output shall remain unchanged for supported files compared to the current behavior.
- [ ] **REQ-009**: Lazy promotion shall apply to injected and nested syntaxes encountered during tokenization.

## Non-Functional Requirements
- **Performance**: Startup should avoid the current full-registry compile cost and only pay compilation costs for syntaxes that are actually used.
- **Reliability**: Lazy promotion shall not introduce inconsistent state between registry lookups, buffer classification, and tokenizer resolution.
- **Compatibility**: Existing syntax definitions, aliases, and theme syntax tags shall continue to work without changes to builtin syntax files or user-facing configuration.
- **Usability**: The user should only observe a faster startup and, at most, a small one-time delay the first time a previously unused syntax is needed.

## Acceptance Criteria
- [ ] **AC-001**: Launching the editor with no files does not eagerly compile all builtin syntaxes before the first frame is shown.
- [ ] **AC-002**: Opening a file with a supported filetype still classifies the buffer correctly and displays the expected syntax label.
- [ ] **AC-003**: A nested or injected syntax, such as a Markdown fenced code block, still highlights using the expected nested language after lazy promotion is enabled.
- [ ] **AC-004**: Re-entering an already promoted nested syntax reuses the compiled definition instead of recompiling it.
- [ ] **AC-005**: Invalid builtin syntax data still produces the same class of load failure as before, but only when the affected syntax is actually promoted.
- [ ] **AC-006**: Manual startup timing shows that the registry compile cost no longer appears on the critical startup path for syntaxes that are not needed immediately.

## Out of Scope
- Rewriting the syntax grammar format.
- Changing syntax highlighting categories or theme keys.
- Implementing background or threaded syntax compilation.
- Hot reloading syntax definitions at runtime.
- Changing user-facing syntax configuration options.

## Assumptions
- The observed startup lag is primarily caused by builtin syntax compilation, not terminal initialization or theme loading.
- It is acceptable for the first use of a previously unused syntax to incur a small one-time promotion cost.
- Nested and injected syntaxes should continue to behave transparently, so promotion must work across tokenizer-driven resolution.
- The current syntax parsing and validation logic can be reused for both eager and lazy paths.

## Dependencies
- Existing syntax registry parsing and validation code in `src/syntax/mod.rs`.
- Buffer syntax classification and tokenizer lookup paths in `src/buffer/io.rs` and `src/buffer/syntax.rs`.
- Existing syntax fixture tests under `src/buffer/tests/syntax/` and registry tests in `src/syntax/mod.rs`.
