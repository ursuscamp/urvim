# Markdown Highlighting Bugs - Implementation Tasks

## Overview

Fix Markdown fenced-block delimiter rendering and remove generic syntax-token
heuristics from the shared tokenizer so syntax highlighting comes only from
explicit grammar rules.

## Backend

- [x] **1.** Fix injected-region delimiter span handling in the shared tokenizer
  - [x] **1.1** Correct the closing fence span so it covers the delimiter bytes instead of the bytes after the delimiter
  - [x] **1.2** Verify the fix applies to all injected regions, not just Markdown fences

- [x] **2.** Remove generic identifier heuristics from syntax tokenization
  - [x] **2.1** Delete capitalization-based fallback styling for identifiers in the shared tokenizer
  - [x] **2.2** Ensure ordinary identifiers are only highlighted when matched by explicit grammar rules such as keywords, constants, types, or region definitions
  - [x] **2.3** Preserve explicit grammar-driven tokenization for comments, strings, regions, numbers, punctuation, and operators

- [x] **3.** Add regression coverage for the tokenizer behavior
  - [x] **3.1** Add a test proving the closing delimiter of a Markdown fenced block is highlighted with the fence style
  - [x] **3.2** Add a test proving Markdown prose words are not styled solely because they are capitalized or SCREAMY_CASE
  - [x] **3.3** Add a test proving non-Markdown syntaxes still highlight explicit keywords and string/comment regions correctly after the heuristic removal

## Testing

- [x] **4.** Run focused and full validation
  - [x] **4.1** Run the targeted syntax and buffer tests covering Markdown fences and explicit grammar highlighting
  - [x] **4.2** Run `cargo check` to verify the build and catch warnings

## Completion Summary

| Area | Status | Notes |
| --- | --- | --- |
| Backend | Complete | Shared tokenizer and delimiter span fixes |
| Testing | Complete | Regression coverage and validation |
| Total | 4/4 complete | 100% |
