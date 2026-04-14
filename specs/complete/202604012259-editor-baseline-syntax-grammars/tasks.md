# Editor Baseline Syntax Grammars - Implementation Tasks

## Overview
Implement full lexical baseline grammars for the editor’s broad language bundle: `typescript`, `html`, `css`, `yaml`, `c`, `cpp`, `go`, and `java`. The work should add grammar coverage, wire up syntax tests, and refresh fixtures so the built-ins are verified at the buffer-highlighting level.

## Backend
- [x] **1.** Audit the current built-in syntax registry and identify the exact files and metadata entries that need updates for the eight target languages.
  - [x] **1.1** Confirm which of the target languages are currently stubs versus partial grammars.
  - [x] **1.2** Confirm the existing filetype aliases and filename mappings that should remain unchanged.
- [x] **2.** Implement the `typescript` grammar baseline, including TSX-compatible handling where the current mapping already routes `.tsx` to TypeScript. (depends on: 1.1, test: TypeScript spans include keywords, types, decorators, and TS-specific declarations)
  - [x] **2.1** Add lexical coverage for comments, strings, numbers, keywords, type syntax, interfaces, enums, decorators, and TS declarations.
  - [x] **2.2** Preserve JSX-like readability for TSX files without requiring full parser accuracy.
  - [x] **2.3** Add or refresh the TypeScript fixture to exercise both common and TS-specific constructs.
- [x] **3.** Implement the `html` grammar baseline with nested script/style delegation. (depends on: 1.1, test: HTML spans include tags, attributes, comments, entities, and nested bodies)
  - [x] **3.1** Add highlighting for tags, attributes, attribute values, comments, doctypes, and entities.
  - [x] **3.2** Delegate `<script>` and `<style>` bodies to the corresponding nested syntax when available.
  - [x] **3.3** Add or refresh the HTML fixture to include embedded script and style examples.
- [x] **4.** Implement the `css` grammar baseline. (depends on: 1.1, test: CSS spans include selectors, at-rules, properties, values, units, and colors)
  - [x] **4.1** Add highlighting for comments, selectors, at-rules, property names, property values, numbers with units, colors, strings, and punctuation.
  - [x] **4.2** Add or refresh the CSS fixture with representative selector and declaration examples.
- [x] **5.** Implement the `yaml` grammar baseline. (depends on: 1.1, test: YAML spans include keys, scalars, block scalars, anchors, aliases, tags, directives, and punctuation)
  - [x] **5.1** Add highlighting for comments, keys, plain scalars, quoted scalars, block scalars, anchors, aliases, tags, directives, and punctuation.
  - [x] **5.2** Add or refresh the YAML fixture with multiline block scalar and collection examples.
- [x] **6.** Implement the `c` grammar baseline. (depends on: 1.1, test: C spans include comments, preprocessor directives, strings, chars, numbers, keywords, builtin types, operators, and punctuation)
  - [x] **6.1** Add highlighting for comments, preprocessor directives, strings, char literals, numeric literals with suffixes, keywords, builtin types, operators, and punctuation.
  - [x] **6.2** Add or refresh the C fixture with preprocessor and literal examples.
- [x] **7.** Implement the `cpp` grammar baseline on top of the C baseline. (depends on: 6.1, test: C++ spans include raw strings, namespaces, templates, boolean/null constants, and C++ keywords)
  - [x] **7.1** Extend the C coverage with raw strings, namespaces, templates, boolean and null constants, and C++ keyword coverage.
  - [x] **7.2** Add or refresh the C++ fixture with raw-string and template examples.
- [x] **8.** Implement the `go` grammar baseline. (depends on: 1.1, test: Go spans include comments, raw strings, interpreted strings, rune literals, numbers, keywords, builtin types, and operators)
  - [x] **8.1** Add highlighting for comments, raw strings, interpreted strings, rune literals, numbers, keywords, builtin types, and punctuation or operators.
  - [x] **8.2** Add or refresh the Go fixture with representative literals and declarations.
- [x] **9.** Implement the `java` grammar baseline. (depends on: 1.1, test: Java spans include comments, doc comments, strings, text blocks, chars, numbers, annotations, keywords, and builtin types)
  - [x] **9.1** Add highlighting for comments, doc comments, strings, text blocks, chars, numbers, annotations, keywords, and builtin types.
  - [x] **9.2** Add or refresh the Java fixture with annotation and multiline text-block coverage.

## Testing
- [x] **10.** Add or update the syntax test modules under `src/buffer/tests/syntax/` so each target language has a dedicated regression test file. (depends on: 2.3, 3.3, 4.2, 5.2, 6.2, 7.2, 8.2, 9.2, test: `cargo test syntax`)
  - [x] **10.1** Add a `typescript` syntax test module and wire it into `src/buffer/tests/syntax/mod.rs`.
  - [x] **10.2** Add any missing modules for `html`, `css`, `yaml`, `c`, `cpp`, `go`, and `java` if the repo does not already provide them.
  - [x] **10.3** Ensure each module loads the corresponding fixture and asserts on the expected tags.
- [x] **11.** Add fixture coverage for multiline and language-specific constructs in `src/buffer/tests/syntax/fixtures/` for every target language. (depends on: 2.3, 3.3, 4.2, 5.2, 6.2, 7.2, 8.2, 9.2, test: fixture lines map to the expected spans)
  - [x] **11.1** Make sure each fixture includes comments, strings, and at least one multiline construct.
  - [x] **11.2** Make sure each fixture includes one language-specific construct that would fail under a plain-text fallback.
- [x] **12.** Run `cargo check` and the syntax-focused test suite, then fix any warnings or regressions discovered. (depends on: 2, 3, 4, 5, 6, 7, 8, 9, test: build and syntax tests pass)
  - [x] **12.1** Verify the project builds cleanly with `cargo check`.
  - [x] **12.2** Verify syntax tests pass for the updated grammars and fixtures.
  - [x] **12.3** Fix any clippy or style issues that surface during validation.

## Completion Summary
| Status | Count | Notes |
| --- | ---: | --- |
| Completed | 12 | Implementation and validation are complete. |
| In Progress | 0 | No active work remains. |
| Blocked | 0 | No known blockers. |
| Pending | 0 | All checklist items are complete. |
