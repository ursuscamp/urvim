# Syntax Config-Driven Regions

## Summary

Move syntax highlighting rules out of Rust code and into per-language TOML configuration files that are loaded into memory like theme configs. The new syntax system should preserve the current highlighting coverage for supported filetypes, add regex-defined regions, support nested delimited regions with child syntax rules, and keep the existing incremental invalidation behavior and performance characteristics.

## Problem Statement

urvim's current syntax highlighting rules are embedded directly in code, which makes them difficult to extend, audit, and evolve independently of the editor binary. The current implementation also hardcodes region logic for comments, strings, Markdown fences, and a few filetype-specific heuristics in a way that is not user-editable. We need a config-driven syntax system that can represent the existing behavior and provide a path to richer region-based highlighting without sacrificing the cache and invalidation behavior that keeps rendering responsive.

## User Stories

- As a user, I want syntax definitions to live in TOML files, so that highlighting behavior can be adjusted without changing the editor source.
- As a contributor, I want one syntax definition per file, so that rules are easy to find, review, and extend.
- As a maintainer, I want the current syntax coverage to remain intact during the migration, so that existing files continue to highlight as expected.
- As a user, I want regions with start and end delimiters to be supported, so that strings, comments, and other nested structures can be highlighted accurately.
- As a user, I want Markdown code fences to inject language-specific highlighting, so that embedded code is highlighted with the right rules.
- As a maintainer, I want incremental invalidation and cached line state to keep working, so that editing large files remains fast.

## Functional Requirements

- [ ] **REQ-001**: The editor shall load syntax definitions from TOML configuration files instead of hardcoding language rules in Rust source.
- [ ] **REQ-002**: Each syntax definition shall live in its own TOML file and be identifiable by a stable syntax name and associated filetype or filetype aliases.
- [ ] **REQ-003**: The editor shall load syntax definition files into memory through a registry-like flow comparable to theme loading.
- [ ] **REQ-004**: The syntax system shall support inline regions that match text using regular expressions or an equivalent pattern-matching mechanism.
- [ ] **REQ-005**: The syntax system shall support delimited regions with a start pattern and end pattern, including regions that may span multiple lines.
- [ ] **REQ-006**: A delimited region shall be able to delegate its interior to a nested syntax definition so that embedded text can be highlighted differently from the container region.
- [ ] **REQ-007**: The syntax system shall support Markdown fenced code blocks as a nested-region use case, including language injection when the fence identifies a known syntax.
- [ ] **REQ-008**: The syntax system shall preserve the current highlight categories used by the renderer: comment, constant, function, keyword, number, operator, punctuation, string, type, and variable.
- [ ] **REQ-009**: The syntax system shall preserve the current supported filetypes that have syntax highlighting today, including Rust, Python, JavaScript, TypeScript, Shell-family files, JSON, TOML, and Markdown.
- [ ] **REQ-010**: The syntax system shall continue to highlight existing language features currently covered by the editor, including line comments, block comments, strings, multiline strings, numeric literals, keywords, types, constants, function names, punctuation, operators, JSON/TOML key names, Markdown headings, inline code, links, and fenced code blocks.
- [ ] **REQ-011**: Syntax highlighting shall remain incremental, such that edits invalidate cached syntax from the affected line forward instead of forcing a full-buffer reparse.
- [ ] **REQ-012**: Changes before or inside a multiline or nested region shall invalidate dependent downstream lines so cached syntax state remains correct after edits.
- [ ] **REQ-013**: The syntax loader shall reject malformed or invalid syntax definition files with a clear error rather than registering partial or corrupt rules.
- [ ] **REQ-014**: Plain text and unsupported filetypes shall continue to render without syntax spans rather than failing or applying incorrect rules.

## Non-Functional Requirements

- **Performance**: Syntax definitions shall be parsed once and reused, and line-by-line highlighting should keep the current incremental cache behavior and avoid reparsing unchanged prefixes.
- **Reliability**: Invalid syntax definitions shall fail clearly, and syntax state should remain consistent after edits that affect nested or multiline regions.
- **Compatibility**: The migration shall preserve the current visible highlighting behavior for existing supported filetypes and syntax categories.
- **Maintainability**: Adding or editing a syntax definition should not require changing the core highlighting engine for routine rule updates.
- **Usability**: Syntax configuration files should be understandable enough that contributors can add or adjust a language rule without navigating hardcoded tokenizer logic.

## Acceptance Criteria

- [ ] **AC-001**: Representative Rust, Python, JavaScript/TypeScript, shell, JSON, TOML, and Markdown samples still highlight the same major token classes after the migration.
- [ ] **AC-002**: A TOML syntax definition can preserve multiline string highlighting across line boundaries.
- [ ] **AC-003**: A Markdown syntax definition can highlight a fenced code block and inject a nested language definition when the fence language is recognized.
- [ ] **AC-004**: A syntax definition can express a region matched by a regular expression or equivalent pattern and render that region with the configured style.
- [ ] **AC-005**: Editing a line that affects a nested or multiline region causes downstream cached syntax to refresh correctly.
- [ ] **AC-006**: Loading an invalid syntax TOML file produces a clear error and does not silently register broken rules.
- [ ] **AC-007**: Unsupported filetypes and plain text continue to render without syntax highlighting rather than erroring.

## Out of Scope

- Runtime syntax hot-reloading after startup
- Semantic or AST-based highlighting
- A UI for editing syntax definition files inside urvim
- Adding brand-new language coverage beyond the current supported set as part of this migration
- Changing theme syntax color categories as part of the syntax rule migration

## Assumptions

- Each syntax definition file will map cleanly to a single syntax name, even if it supports multiple filetype aliases.
- Regular expressions will be sufficient for the initial region-matching model, unless a clearly superior representation is needed to preserve current behavior.
- Markdown code fence injection can resolve nested syntax by language name or filetype alias.
- The current syntax category palette in themes remains the canonical set of renderer styles.

## Dependencies

- Existing filetype detection and buffer syntax cache behavior
- Existing theme syntax style keys and renderer span application
- Existing config loading conventions for TOML-backed registries
- A regex engine or equivalent pattern-matching implementation in the Rust codebase
