# Rust Format String Syntax Highlighting

## Summary
urvim should highlight Rust format strings using the standard `std::fmt` format-string syntax so that formatting macro calls show distinct styling for the macro name, call punctuation, literal text, placeholder regions, and escape sequences.

## Problem Statement
Rust formatting macros use a dedicated string syntax that is richer than an ordinary string literal. The current highlighting behavior needs a precise, context-aware model of that syntax so format strings can be styled consistently without applying placeholder parsing to ordinary strings.

## User Stories
- As a Rust developer, I want format strings to visually distinguish literal text from placeholder regions, so that I can read formatted output quickly.
- As a Rust developer, I want escaped braces like `{{` and `}}` to remain part of the string content, so that literal braces are not mistaken for placeholders.
- As a Rust developer, I want named, positional, and implicit placeholders to be highlighted clearly, so that I can recognize which parts of the format string are interpreted by `std::fmt`.
- As a maintainer, I want ordinary Rust strings to keep their existing highlighting, so that the format-string rules do not leak into non-format text.

## Functional Requirements
- [ ] **REQ-001**: The editor shall apply format-string highlighting only when a Rust string literal appears in a recognized formatting-macro call context.
- [ ] **REQ-002**: The editor shall keep ordinary Rust string literals outside formatting contexts on the existing string-highlighting path.
- [ ] **REQ-003**: The editor shall highlight format-string literal text differently from placeholder regions enclosed by braces.
- [ ] **REQ-004**: The editor shall treat escaped brace pairs `{{` and `}}` as literal format-string content rather than placeholder delimiters.
- [ ] **REQ-005**: The editor shall highlight placeholder forms defined by Rust `std::fmt`, including implicit `{}`, positional `{0}`, and named `{name}` references.
- [ ] **REQ-006**: The editor shall highlight format-specifier content after `:` inside a placeholder distinctly from the surrounding literal string text.
- [ ] **REQ-007**: The editor shall preserve existing highlighting for other Rust syntax inside the format-string context where the grammar already supports it, such as placeholder identifiers or nested subregions.
- [ ] **REQ-008**: The editor shall support the standard Rust formatting syntax used by macros such as `format!`, `println!`, `print!`, `eprintln!`, `eprint!`, `write!`, `writeln!`, and `format_args!` when they accept a leading format-string literal.
- [ ] **REQ-009**: The editor shall continue to update format-string highlighting after edits without requiring a restart.
- [ ] **REQ-010**: The format-string highlighting change shall not alter buffer contents, undo history, save behavior, or cursor motion.

## Non-Functional Requirements
- **Compatibility**: The feature shall work with the current syntax highlighting, theme, and buffer rendering systems.
- **Reliability**: Highlighting shall remain correct for repeated edits inside and around format strings.
- **Usability**: Placeholder and escape syntax shall be visually distinguishable without additional configuration.
- **Maintainability**: Rust format-string behavior shall remain isolated to the Rust syntax definition and shared syntax-engine support.

## Acceptance Criteria
- [ ] **AC-001**: A Rust file containing `format!("Hello, {}!", name)` highlights the macro invocation, the format string, and the placeholder region with distinct syntax categories.
- [ ] **AC-002**: A Rust file containing `format!("{name:04}")` highlights the named placeholder and the format-specifier portion inside the braces.
- [ ] **AC-003**: A Rust file containing `format!("{{literal}}")` treats the escaped braces as literal string content rather than placeholder regions.
- [ ] **AC-004**: A Rust file containing `let s = "hello {name}";` keeps ordinary string highlighting and does not apply format-string placeholder styling.
- [ ] **AC-005**: Editing text inside or adjacent to a format-string call updates the rendered highlighting without a manual refresh.
- [ ] **AC-006**: Syntax definitions that do not opt into Rust format-string rules continue to render strings normally.

## Out of Scope
- Semantic validation of whether a format string is compile-time correct.
- Changing Rust runtime formatting behavior.
- Parser-backed highlighting for full Rust expressions inside placeholders.
- User-configurable format-string detection rules.

## Assumptions
- The built-in Rust syntax definition can identify formatting-macro call contexts well enough to apply format-string rules to the first string literal argument.
- The existing theme system already supports the tags needed to style strings, punctuation, and nested placeholder regions.
- The syntax engine can represent placeholder regions and escaped delimiters without introducing a new renderer API.

## Dependencies
- Existing context-aware syntax highlighting support.
- Existing Rust builtin syntax definition and syntax-loader infrastructure.
- Existing syntax tag vocabulary for `string`, `punctuation`, `variable`, and related child tags.
- Regression test coverage for Rust syntax highlighting.
