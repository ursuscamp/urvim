# Context-Driven Syntax Engine

## Summary
Rework urvim's syntax engine so regex rules plus context state become the primary grammar primitive. Context should control which rules are active, which embedded syntax is selected, and when the tokenizer should switch between host-language and nested-language behavior.

## Problem Statement
The current syntax engine relies on a mix of fixed region shapes and special nested constructs to express multi-step parsing behavior. That works for some cases, but it makes structured languages like HTML harder to model cleanly and encourages special-case handling for embedded content. A context-driven engine would let grammars express opener, body, and closer behavior as explicit state transitions instead of forcing them into predefined region categories.

## User Stories

- As a contributor, I want syntax behavior to be driven by regex rules and context stacks, so that grammar definitions stay composable and easier to reason about.
- As a maintainer, I want embedded-language behavior to arise from active context, so that HTML-like constructs can be modeled without special-case region branches.
- As a user editing structured markup, I want openers, attributes, bodies, and closers to highlight consistently, so that complex files remain readable.
- As a contributor, I want the engine to start with a small primitive set and grow specialized rules only when repeated patterns justify them, so that the syntax system stays maintainable.

## Functional Requirements

- [ ] **REQ-001**: The syntax engine shall support grammar behavior built from regex rules and context state as the primary parsing primitive.
- [ ] **REQ-002**: Context markers shall control which rules are eligible to match at a given scan position.
- [ ] **REQ-003**: The syntax engine shall support pushing and popping named context markers as part of rule execution.
- [ ] **REQ-004**: The syntax engine shall support nested syntax selection from active context without requiring a predefined special-purpose region kind for each embedded language case.
- [ ] **REQ-005**: The syntax engine shall continue to support multi-line highlighting state across line boundaries.
- [ ] **REQ-006**: The syntax engine shall preserve incremental caching and invalidation behavior when context changes affect downstream lines.
- [ ] **REQ-007**: The syntax engine shall continue to support host-language highlighting around embedded-language bodies, including opener and closer text, without requiring raw delimiter-only handling.
- [ ] **REQ-008**: Existing built-in syntaxes shall continue to render with their current major highlight categories unless a change is required to express the new context-driven model.
- [ ] **REQ-009**: The syntax loader shall reject invalid regexes, malformed context metadata, and invalid nested-syntax references with clear errors.
- [ ] **REQ-010**: The syntax system shall remain data-driven and shall not require parser-only or AST-based highlighting to achieve the refactor.

## Non-Functional Requirements

- **Maintainability**: The core syntax model should remain small and composable, with specialized behavior added only after repeated patterns emerge.
- **Compatibility**: The refactor should preserve existing filetype resolution, theme tag mapping, and buffer rendering APIs.
- **Reliability**: Highlighting should remain deterministic across edits and multiline constructs.
- **Performance**: Incremental line-by-line tokenization and cache reuse should remain intact after the refactor.

## Acceptance Criteria

- [ ] **AC-001**: A grammar can use regex rules and context transitions to model an opener/body/closer sequence without relying on a bespoke region type for that exact construct.
- [ ] **AC-002**: Context membership determines which rules are active, and context transitions are reflected correctly in downstream lines.
- [ ] **AC-003**: Embedded language bodies can be selected from context-driven state and continue highlighting across multiple lines.
- [ ] **AC-004**: Existing supported syntaxes still load and highlight correctly after the tokenizer refactor.
- [ ] **AC-005**: Cache invalidation still recomputes affected lines correctly when edits change the active context.

## Out of Scope

- Adding a full parser or AST-based syntax engine
- Implementing every specialized region shape up front
- Reworking theme definitions or adding new syntax tag categories
- Changing filetype detection behavior beyond what the refactor requires internally

## Assumptions

- Regex matching plus context transitions are sufficient as the first-stage primitive set.
- Existing nested syntax resolution can be adapted to context-driven state without changing the public renderer APIs.
- Specialized region forms, if still needed later, should be derived from repeated real patterns rather than introduced preemptively.

## Dependencies

- Existing syntax registry and loader
- Existing buffer-owned syntax cache and invalidation flow
- Existing theme tag vocabulary
- Existing syntax fixtures and regression tests
