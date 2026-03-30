# Syntax Correctness Holes 1

## Summary
urvim should close the biggest correctness gaps in the existing partial built-in grammars so that the most common source-language files are highlighted accurately enough to be useful during normal editing.

## Problem Statement
Several built-in syntaxes already exist, but the current coverage leaves obvious lexical holes that make common code and structured text misleading to read. In particular, the existing partial grammars for Rust, Python, JavaScript, JSON, TOML, Markdown, and shell-family text do not yet handle the most important language constructs consistently enough for everyday editing.

Without these corrections, the editor can mis-highlight valid syntax as plain text, over-highlight invalid forms, or fail to keep multiline constructs stable across lines. That makes the affected filetypes feel unfinished and can hide mistakes in the text being edited.

## User Stories
- As a programmer, I want Rust, Python, and JavaScript files to highlight the basic lexical forms of the language correctly, so that I can read and edit code without obvious syntax glitches.
- As someone working with structured data, I want JSON and TOML files to distinguish valid literals, keys, numbers, and strings correctly, so that malformed or missing structure stands out.
- As someone editing prose or documentation, I want Markdown to highlight common block and inline constructs correctly, so that headings, lists, code blocks, links, and emphasis are easy to scan.
- As someone editing shell scripts, I want command text, strings, expansions, and heredoc-style regions to stay readable across lines, so that shell scripts do not collapse into misleading plain text.

## Functional Requirements
- [ ] **REQ-001**: Rust highlighting shall distinguish the main lexical token families used in ordinary Rust source, including comments, strings, character-like literals, numeric literals, keywords, identifiers, attributes, lifetimes or labels, and macros where they are visibly distinguishable.
- [ ] **REQ-002**: Rust highlighting shall keep multiline literal forms stable across line boundaries, including raw strings and other delimiter-based string forms that are valid Rust text.
- [ ] **REQ-003**: Python highlighting shall distinguish the main lexical token families used in ordinary Python source, including comments, string literals, raw or prefixed strings, numeric literals, keywords, identifiers, decorators, and formatted-string regions where they are visibly distinguishable.
- [ ] **REQ-004**: Python highlighting shall keep multiline string forms stable across line boundaries and shall not misclassify common prefixed literal forms as plain identifiers.
- [ ] **REQ-005**: JavaScript highlighting shall distinguish the main lexical token families used in ordinary JavaScript source, including comments, string literals, template literals, numeric literals, keywords, identifiers, regex literals where supported, and class or property-related lexical forms where they are visibly distinguishable.
- [ ] **REQ-006**: JavaScript highlighting shall keep multiline literal and template forms stable across line boundaries and shall highlight interpolation regions distinctly when they appear inside template literals.
- [ ] **REQ-007**: JSON highlighting shall reject non-JSON identifier-like text as a valid structural token and shall highlight only valid JSON lexical forms, including strings, numbers, punctuation, booleans, and null.
- [ ] **REQ-008**: JSON highlighting shall recognize the full range of valid JSON number forms, including negative values and exponent notation.
- [ ] **REQ-009**: TOML highlighting shall recognize the full range of valid TOML number forms, including signed integers, floats, underscores, base-prefixed integers, and exponent forms.
- [ ] **REQ-010**: TOML highlighting shall distinguish keys, table headers, arrays of tables, inline tables, strings, and comments in a way that makes ordinary TOML structure visually clear.
- [ ] **REQ-011**: Markdown highlighting shall distinguish the core block and inline constructs used in everyday Markdown, including headings, lists, blockquotes, fenced code blocks, indented code blocks, links, images, emphasis, strong emphasis, and reference-style constructs where supported.
- [ ] **REQ-012**: Markdown highlighting shall preserve multiline block constructs across line boundaries so that fences, quotes, lists, and code blocks remain visually coherent while editing.
- [ ] **REQ-013**: Shell highlighting shall distinguish shell comments, words, strings, expansions, command substitutions, arithmetic substitutions, heredoc-like regions, and keywords or builtins where they are visually distinguishable.
- [ ] **REQ-014**: Shell highlighting shall preserve multiline shell constructs across line boundaries, including quoted strings and heredoc-style regions, so that scripted text remains stable while editing.
- [ ] **REQ-015**: The updated grammars shall continue to render unsupported or intentionally unimplemented constructs as ordinary text rather than producing misleading syntax categories.

## Non-Functional Requirements
- [ ] **NFR-001**: The corrected highlighting rules shall remain responsive during normal editing and scrolling.
- [ ] **NFR-002**: The updated grammars shall remain reliable under repeated edits, including edits near the start of a file and edits that change line boundaries.
- [ ] **NFR-003**: The updated grammars shall remain compatible with the current theme and filetype systems.
- [ ] **NFR-004**: The updated grammars shall be testable through grammar-driven regression fixtures.

## Acceptance Criteria
- [ ] **AC-001**: Rust, Python, JavaScript, JSON, TOML, Markdown, and shell files show visibly improved lexical accuracy compared with the current partial grammars.
- [ ] **AC-002**: Valid literals and block forms in the supported grammars remain highlighted consistently after insertions, deletions, and line wraps.
- [ ] **AC-003**: Invalid or unsupported identifier-like forms in JSON do not appear as valid JSON structure tokens.
- [ ] **AC-004**: Common Markdown constructs such as headings, fences, links, emphasis, and blockquotes are visually distinct from surrounding text.
- [ ] **AC-005**: Common shell quoted strings and heredoc-style regions remain highlighted consistently across multiple lines.
- [ ] **AC-006**: Regression coverage exists for the grammar corrections covered by this spec.

## Out of Scope
- Replacing the current regex-and-region syntax engine with a parser-backed highlighter.
- Semantic highlighting or language-server integration.
- Perfect disambiguation of every language edge case.
- Expanding the remaining metadata-only stub grammars not named in this spec.
- User-defined syntax configuration.

## Assumptions
- The current syntax engine can express the lexical corrections in this bundle without introducing parser-only features.
- The existing built-in syntax definitions for Rust, Python, JavaScript, JSON, TOML, Markdown, and shell are the starting point for these fixes.
- The theme system already has the syntax tag categories needed for these corrections or can inherit reasonable broader categories when necessary.
- Regression fixtures can represent the intended lexical behavior for each language in this bundle.

## Dependencies
- Existing built-in syntax definitions for Rust, Python, JavaScript, JSON, TOML, Markdown, and shell.
- Existing syntax rendering pipeline.
- Existing syntax regression test harness and fixture format.
- Existing filetype detection and built-in syntax metadata.
