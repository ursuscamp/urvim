# Syntax Tags

## Summary
urvim should let syntax grammar rules emit hierarchical tags instead of directly naming styles. Themes should map those tags to styles, with lookup falling back from the most specific tag to broader parent tags until a match is found. For example, a grammar may tag an integer as `constant.integer` and a float as `constant.float`, while a theme can style `constant` generally and override only `constant.integer` for special cases.

## Problem Statement
The current syntax model couples grammar rules to theme style names. That makes grammars depend on theme-specific style slots, limits how finely a theme can distinguish related syntax categories, and forces theme authors to repeat broad styling across many separate keys. A tag-based model separates syntax meaning from visual styling, so grammars can describe what a token is while themes decide how broad or specific the styling should be.

## User Stories
- As a grammar author, I want each rule to emit a single tag, so that syntax categories are described independently of theme styling.
- As a theme author, I want to style `constant` broadly and override `constant.integer` separately, so that related syntax categories can share a base appearance while still supporting special cases.
- As a user, I want unresolved tags to fall back to broader styles, so that a theme can remain readable even when it does not explicitly style every tag emitted by a grammar.
- As a maintainer, I want a documented base tag vocabulary, so that grammar and theme authors have a shared starting point for common syntax concepts.

## Functional Requirements
- [ ] **REQ-001**: Each syntax grammar rule shall correspond to exactly one tag when it matches text.
- [ ] **REQ-002**: Syntax tags shall be hierarchical, dot-separated identifiers such as `constant.integer` and `constant.float`.
- [ ] **REQ-003**: The editor shall validate tags as lowercase identifiers and reject malformed tags.
- [ ] **REQ-004**: Syntax grammar files shall be able to define arbitrary tags, provided they satisfy the tag validation rules.
- [ ] **REQ-005**: Themes shall map styles to tags rather than to a fixed set of predefined syntax style keys.
- [ ] **REQ-006**: Theme style resolution shall prefer the most specific matching tag, then progressively broader parent tags, and finally the theme default style if no tag match exists.
- [ ] **REQ-007**: When a grammar emits `constant.integer`, a theme-defined `constant.integer` style shall take precedence over a theme-defined `constant` style.
- [ ] **REQ-008**: The editor shall document a standard set of base tags for common syntax concepts in `docs/syntax/tags.md`, including concepts such as constants, strings, comments, keywords, types, identifiers, punctuation, operators, numbers, booleans, and null values.
- [ ] **REQ-009**: Built-in syntax definitions shall migrate from direct style keys to tag-based rule output.
- [ ] **REQ-010**: Built-in themes shall migrate to tag-based style mappings and continue to render supported syntax with appropriate fallback behavior.
- [ ] **REQ-011**: The tag hierarchy shall affect theme style resolution only and shall not change how syntax rules match text.
- [ ] **REQ-012**: A grammar rule shall not emit more than one tag for a single matched span.

## Non-Functional Requirements
- **Compatibility**: The new tag system shall replace the old grammar-to-style coupling without requiring backward compatibility with the pre-release style-key scheme.
- **Usability**: Theme authors shall be able to style broad categories once and override narrower categories only when needed.
- **Maintainability**: The tag vocabulary shall remain extensible so new syntax categories can be added without redesigning the theme system.
- **Reliability**: Invalid tags and unresolved mappings shall fail or fall back deterministically instead of producing ambiguous rendering.

## Acceptance Criteria
- [ ] **AC-001**: A grammar rule tagged `constant.integer` is rendered using the theme's exact `constant.integer` style when that style exists.
- [ ] **AC-002**: A grammar rule tagged `constant.integer` falls back to the theme's `constant` style when `constant.integer` is not defined.
- [ ] **AC-003**: A grammar rule tagged `constant.integer` falls back to the theme default style when neither `constant.integer` nor `constant` is defined.
- [ ] **AC-004**: A malformed tag such as `Constant.Integer`, `constant..integer`, or `.constant` is rejected during validation.
- [ ] **AC-005**: A builtin grammar and builtin theme pair load successfully after migrating to tag-based syntax styling.
- [ ] **AC-006**: The documented base tag vocabulary includes both broad categories and at least one narrower example such as `constant.integer`.
- [ ] **AC-007**: A grammar may use an arbitrary valid tag outside the documented base vocabulary, and a theme may still style it through the same resolution rules.

## Out of Scope
- Backward compatibility with the old fixed syntax style-key mapping format.
- Runtime theme switching after startup.
- Automatic inference of tags from grammar rules.
- Semantic highlighting from language servers.
- Multiple tags per syntax rule.

## Assumptions
- The tag hierarchy is used only for theme lookup, not for syntax matching behavior.
- Existing built-in themes and builtin syntax definitions can be updated together because the project has not shipped a public release yet.
- The standard base tag vocabulary is a documented convention rather than a hardcoded whitelist.
- Theme fallback resolution will stop at the theme default style when no matching tag exists at any specificity level.

## Dependencies
- Existing syntax grammar loading and validation.
- Existing theme loading and style resolution.
- Built-in syntax definitions.
- Built-in theme definitions.
- Regression tests that cover syntax rendering and theme fallback behavior.
