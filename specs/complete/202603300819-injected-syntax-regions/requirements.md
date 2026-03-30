# Injected Syntax Regions

## Summary
urvim should support injected syntax as a first-class syntax-region capability so that a region can delegate its interior text to another syntax definition based on captured text from the region opener. The initial motivating case is Markdown fenced code blocks, but the behavior should apply to any syntax region that can identify a nested language from captured opener text or an equivalent embedded identifier.

## Problem Statement
Today, injected-language highlighting is handled as special-case logic in the syntax highlighter rather than as a region capability described by syntax data. That makes Markdown fenced code blocks harder to extend, keeps nested language selection tied to hardcoded behavior, and limits reuse for other region types that may need the same pattern later. We need a spec-level contract for injected syntax so the highlighter can resolve nested definitions dynamically while preserving existing incremental highlighting behavior and fallback behavior when no nested language is recognized.

## User Stories

- As a writer of Markdown, I want fenced code blocks to highlight with the language I specify in the fence, so that embedded code is easier to read.
- As a contributor, I want injected syntax to be defined as a region capability, so that the same mechanism can support more than Markdown fences.
- As a maintainer, I want unknown or missing fence languages to fall back safely, so that the editor never breaks highlighting because a capture is unrecognized.
- As a user editing large files, I want injected syntax to preserve incremental highlighting, so that nested language regions remain responsive after edits.

## Functional Requirements

- [ ] **REQ-001**: The syntax system shall allow a region to declare that its interior should be highlighted using a nested syntax definition.
- [ ] **REQ-002**: The nested syntax for a region shall be selectable from captured text present in the region opener, such as a language identifier or embedded selector.
- [ ] **REQ-003**: The nested syntax resolver shall support matching captured text to an existing syntax definition by syntax name or filetype alias.
- [ ] **REQ-004**: Markdown fenced code blocks shall be representable through the injected-syntax region model without relying on Markdown-specific hardcoded highlighting logic.
- [ ] **REQ-005**: When a fence or region opener specifies a known nested syntax, the text inside the region shall be highlighted using that nested syntax until the region closes.
- [ ] **REQ-006**: The syntax system shall allow each injected-syntax region to choose how unresolved nested syntax is handled.
- [ ] **REQ-007**: When an injected-syntax region is configured to fall back to the parent syntax, unresolved nested syntax shall render using the enclosing region's styling instead of failing.
- [ ] **REQ-008**: When an injected-syntax region is configured to treat unresolved nested syntax as unstyled, unresolved text inside the region shall render without nested highlighting.
- [ ] **REQ-009**: Markdown fenced code blocks shall treat unknown, missing, or empty captured language identifiers as unstyled rather than inheriting the parent syntax.
- [ ] **REQ-010**: Injected syntax shall preserve line-continuation state across line boundaries for nested regions that span multiple lines.
- [ ] **REQ-011**: Edits before or inside an injected region shall invalidate any dependent cached syntax data so downstream lines are recomputed correctly.
- [ ] **REQ-012**: Injected syntax shall continue to produce the existing syntax style categories used by the renderer.
- [ ] **REQ-013**: The syntax loader shall reject malformed injected-region definitions with a clear error rather than registering partial or corrupt rules.
- [ ] **REQ-014**: Plain text and unsupported filetypes shall continue to render without injected syntax behavior.

## Non-Functional Requirements

- **Performance**: Nested syntax resolution shall remain incremental and reuse cached line state so that injected regions do not require full-buffer reparsing on every edit.
- **Reliability**: The syntax system shall handle unknown tags, missing tags, and invalid injected-region definitions deterministically.
- **Compatibility**: Existing supported filetypes and existing syntax style keys shall continue to work with no change to theme definitions.
- **Maintainability**: The injected-syntax model shall be reusable for future region types and shall not require separate hardcoded logic for each syntax that uses it.

## Acceptance Criteria

- [ ] **AC-001**: A Markdown fenced code block tagged `rust` highlights the body using Rust syntax categories.
- [ ] **AC-002**: A Markdown fenced code block tagged with a known alias such as `js` or `ts` resolves to the matching registered syntax.
- [ ] **AC-003**: A Markdown fenced code block with an unknown captured language identifier renders unstyled inside the fence body and does not error.
- [ ] **AC-004**: A Markdown fenced code block without a captured language identifier renders unstyled inside the fence body.
- [ ] **AC-005**: A multiline injected region preserves nested highlighting or parent-style fallback across line boundaries until the closing delimiter is reached.
- [ ] **AC-006**: Editing text before or inside an injected region refreshes all affected downstream highlight state.
- [ ] **AC-007**: Loading an invalid injected-region syntax definition produces a clear failure and does not partially register the syntax.
- [ ] **AC-008**: Existing syntax highlighting for supported filetypes continues to render with the same major categories after injected syntax is enabled.

## Out of Scope

- Parser-driven highlighting such as tree-sitter
- Runtime hot-reloading of syntax definitions
- A UI for editing syntax definitions inside the editor
- New theme syntax categories
- General syntax-registry migration work unrelated to injected syntax

## Assumptions

- The syntax registry already provides a stable way to resolve a syntax definition by name or filetype alias.
- Markdown fences are the first and most visible injected-syntax use case, but the feature should not be encoded as Markdown-only behavior in the spec.
- Unknown or missing nested syntax identifiers should be treated as a fallback case, not as a hard error.
- The current theme syntax category palette remains the canonical renderer vocabulary.

## Dependencies

- Existing syntax registry and filetype lookup behavior
- Existing buffer-owned syntax cache and invalidation flow
- Existing theme syntax style keys
- Existing Markdown filetype support
