# Lazy Syntax Promotion - Implementation Tasks

## Overview
Implement lazy promotion for builtin syntax definitions so startup only parses syntax metadata, while top-level syntax resolution and nested/injected syntax tokenization promote compiled syntaxes on demand.

## Backend
- [x] **1.** Refactor the builtin syntax registry to store raw and compiled syntax entries side by side. 
  - [x] **1.1** Introduce an internal entry type that can hold parsed raw syntax data before promotion and a compiled syntax after promotion.
  - [x] **1.2** Keep metadata indexes for canonical names, aliases, filename patterns, and shebang patterns working from the raw representation.
  - [x] **1.3** Add a promotion path that compiles a raw syntax exactly once and caches the compiled result for later lookups.
- [x] **2.** Update top-level syntax selection to use the promotion-aware registry path without reintroducing eager full-registry compilation.
  - [x] **2.1** Ensure buffer creation and syntax refresh still classify untitled buffers and file-backed buffers correctly.
  - [x] **2.2** Preserve existing fallback behavior to `plaintext` when no syntax matches.
  - [x] **2.3** Keep syntax labels stable after promotion so the status bar and buffer metadata continue to display the same values.
- [x] **3.** Update nested/injected syntax resolution so promotion happens when a tokenizer first encounters a nested language.
  - [x] **3.1** Route injected syntax selector resolution through the same promotion-aware registry path.
  - [x] **3.2** Store the promoted nested syntax in tokenizer state so repeated encounters reuse the compiled definition.
  - [x] **3.3** Preserve existing fallback behavior for unresolved injected syntax selectors.

## Testing
- [x] **4.** Add registry tests for lazy promotion behavior.
  - [x] **4.1** Verify builtin syntax parsing succeeds without forcing every syntax to compile immediately.
  - [x] **4.2** Verify a top-level lookup promotes one syntax and later lookups reuse the cached compiled entry.
  - [x] **4.3** Verify invalid syntax data still fails deterministically when promotion is attempted.
- [x] **5.** Add buffer/tokenizer regression coverage for nested and injected syntax promotion.
  - [x] **5.1** Verify a Markdown fenced code block still resolves and highlights the nested syntax correctly.
  - [x] **5.2** Verify re-entering the same nested syntax reuses the compiled nested entry.
  - [x] **5.3** Verify unsupported or unknown injected selectors continue to fall back as before.
- [x] **6.** Run project checks and targeted verification.
  - [x] **6.1** Run `cargo check` and fix any compile or warning regressions.
  - [x] **6.2** Run focused syntax and buffer tests that cover top-level and nested syntax classification.
  - [x] **6.3** Perform a manual startup smoke test and confirm the startup log no longer shows the full registry compile cost on the critical path.

## Documentation
- [x] **7.** Update syntax system documentation to describe lazy promotion behavior where relevant.
  - [x] **7.1** Review `docs/syntax/highlighting.md` for any description that assumes eager compilation of all syntaxes.
  - [x] **7.2** Update `docs/syntax/grammar.md` if the injected/nested syntax flow needs a note about on-demand promotion.
  - [x] **7.3** Keep the docs aligned with the implemented top-level and injected syntax resolution behavior.

## Completion Summary
| Area | Status | Notes |
| --- | --- | --- |
| Registry refactor | Complete | Raw/compiled entry model and promotion cache |
| Top-level selection | Complete | Buffer classification and fallback behavior |
| Nested/injected promotion | Complete | Tokenizer-driven lazy promotion |
| Testing | Complete | Registry and syntax regression coverage |
| Documentation | Complete | Syntax docs updates |
