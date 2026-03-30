# Context-Driven Syntax Engine - Implementation Tasks

## Overview

Total: 6 tasks
Estimated completion: 3-5 days
Prerequisites: Approved requirements and design

## Backend

- [x] **1.** Rework the syntax schema and loader around an ordered `rules` list with regex and injection entries
  - [x] **1.1** Replace the current region-centric schema shapes with a single ordered rules list that supports `regex` and `injection` variants.
  - [x] **1.2** Preserve context metadata on each rule so rules can require, push, and pop markers during tokenization.
  - [x] **1.3** Validate nested syntax selectors, fallback policies, and malformed regexes at load time with clear syntax-level errors.
  - [x] **1.4** Update the builtin grammar parsing path so existing syntax files can express hierarchical rule order without named rule sections.

- [x] **2.** Refactor the tokenizer so context state drives rule eligibility and nested body delegation
  - [x] **2.1** Make the tokenizer consult the active context stack before attempting each rule match.
  - [x] **2.2** Apply context push/pop transitions as part of rule execution and preserve the resulting state across lines.
  - [x] **2.3** Implement `injection` rule handling so a matching context can delegate the current body to a nested syntax definition.
  - [x] **2.4** Ensure closing host rules preempt nested delegation by ordering and context checks.
  - [x] **2.5** Keep cache invalidation and per-line recomputation deterministic when context changes affect downstream lines.

- [x] **3.** Migrate builtin grammars that currently rely on region special cases onto the new rule model
  - [x] **3.1** Update HTML to model tag scanning, host-tag detection, embedded body delegation, and closing tags with the new rules list.
  - [x] **3.2** Update Markdown fenced code blocks to use capture-based injection rules with explicit closing-fence context handling.
  - [x] **3.3** Review other builtins that rely on special nested-region behavior and convert any that can now be expressed with regex plus context.

## Testing

- [x] **4.** Add and update regression coverage for the new rule model
  - [x] **4.1** Add loader tests for valid `regex` and `injection` rules, invalid regexes, invalid selectors, and malformed context data.
  - [x] **4.2** Add tokenizer tests that verify context-gated matching, push/pop behavior, and nested delegation across line boundaries.
  - [x] **4.3** Add HTML syntax tests that assert tags, attributes, host openers/closers, and embedded JavaScript/CSS bodies all highlight correctly.
  - [x] **4.4** Add Markdown fence tests that verify known captures, unknown captures, and closing-fence precedence.

- [x] **5.** Update syntax documentation to describe the new model clearly
  - [x] **5.1** Revise `docs/syntax/grammar.md` to explain the ordered `rules` list, `regex` rules, `injection` rules, and context transitions.
  - [x] **5.2** Revise `docs/syntax/highlighting.md` where it describes tokenizer behavior so the explanation matches the new rule-driven flow.
  - [x] **5.3** Update any examples that still describe named rule sets or region-centric behavior.

- [x] **6.** Verify the refactor with build, lint, and test checks
  - [x] **6.1** Run `cargo check` and fix compile errors or warnings introduced by the refactor.
  - [x] **6.2** Run the focused syntax and buffer test subset covering the migrated grammars.
  - [x] **6.3** Run the full test suite if the focused checks pass cleanly.
  - [x] **6.4** Fix any clippy issues surfaced by the refactor before finishing.

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Backend | 3 | 3 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **6** | **6** | **100%** |
