# Context-Sensitive Highlighting Engine - Implementation Tasks

## Overview
This work adds a general context-sensitive highlighting capability to the syntax engine, then uses it to correctly highlight Rust formatting strings as the first concrete consumer.

## Backend
- [x] **1.** Extend the syntax schema and engine state model to represent context-sensitive highlighting rules and active context frames.
  - [x] **1.1** Add generalized syntax metadata for context-sensitive regions, including context predicates or equivalent opener-dependent matching data.
  - [x] **1.2** Update the tokenizer state to carry active context frames across line boundaries and nested regions.
  - [x] **1.3** Add bounded re-evaluation/backtracking so edits can rebuild syntax from the earliest affected line without losing surrounding context.
  - [x] **1.4** Preserve current behavior for existing syntax definitions that do not opt into context-sensitive rules.

- [x] **2.** Wire the Rust builtin syntax definition to use the new context-sensitive capability for formatting strings.
  - [x] **2.1** Model the Rust formatting-call opener so the macro or callee name is tagged as `function` or `function.macro`.
  - [x] **2.2** Tag call punctuation as `punctuation` and activate the specialized formatting-string rule set only for the first string argument.
  - [x] **2.3** Keep ordinary Rust string literals on the existing string path when they are not inside a formatting-call context.
  - [x] **2.4** Update syntax tag documentation if needed so the intended `function.macro` usage is discoverable.

## Testing
- [x] **3.** Add regression coverage for the new context-sensitive engine behavior.
  - [x] **3.1** Add loader/validation tests for the new syntax schema or context-sensitive declarations.
  - [x] **3.2** Add syntax highlighting tests that verify Rust formatting calls highlight the macro name, punctuation, format string, and placeholder regions correctly.
  - [x] **3.3** Add a regression test proving ordinary Rust strings are not treated as formatting strings when they appear outside the formatting-call context.
  - [x] **3.4** Add an edit-invalidation test to ensure highlight state is rebuilt correctly after changes near a formatting call.

## Completion Summary
| Area | Total | Done | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 2 | 2 | 0 |
| Testing | 1 | 1 | 0 |
| Total | 3 | 3 | 0 |
