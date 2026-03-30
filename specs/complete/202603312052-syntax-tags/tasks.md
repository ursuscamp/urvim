# Syntax Tags - Implementation Tasks

## Overview

Total: 7 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Backend

- [x] **1.** Add tag validation and hierarchy primitives to the syntax and theme layers
  - [x] **1.1** Introduce a public tag type or equivalent validation helper that accepts lowercase dot-separated identifiers and rejects malformed input
  - [x] **1.2** Add parent-chain resolution helpers so theme lookup can walk from the most specific tag to broader parent tags
  - [x] **1.3** Add documentation comments for the new public tag API and any exported helpers

- [x] **2.** Refactor syntax parsing to emit tags instead of closed syntax style keys
  - [x] **2.1** Replace syntax rule style-key fields with tag fields in the raw and resolved syntax models
  - [x] **2.2** Update syntax loading to validate tags during startup and reject malformed tag values
  - [x] **2.3** Update syntax loader errors and display text so invalid tags report the syntax name and offending tag clearly
  - [x] **2.4** Update built-in syntax TOML sources so grammar rules emit the new canonical and recommended tags

- [x] **3.** Refactor theme syntax resolution to map tag paths to styles
  - [x] **3.1** Replace the closed syntax style-key schema with tag-path style mappings in the theme schema and loader
  - [x] **3.2** Implement specificity-based lookup so exact tag matches win before parent tags and parent tags win before the default style
  - [x] **3.3** Keep UI style resolution unchanged while separating it cleanly from syntax tag resolution
  - [x] **3.4** Update built-in theme TOML files so they define styles for the new canonical tag vocabulary

- [x] **4.** Finalize the documented syntax tag vocabulary
  - [x] **4.1** Create `docs/syntax/tags.md` as the canonical user-facing reference for syntax tags
  - [x] **4.2** Document the canonical top-level tags, the recommended child tags, and the fallback/specificity rules
  - [x] **4.3** Include the `markup` family guidance, including the `.text` refinement convention for interior markup text
  - [x] **4.4** Update `specs/glossary.md` or related project terminology if any new term definitions are needed

## Rendering

- [x] **5.** Wire tag-based syntax styling through rendering and buffer syntax spans
  - [x] **5.1** Update syntax span data to carry tags instead of syntax style keys
  - [x] **5.2** Update render paths to request a final style from the active theme using the span tag
  - [x] **5.3** Ensure unsupported or unmatched tags fall back to the theme default style without breaking rendering
  - [x] **5.4** Keep cursor, gutter, scrolling, and empty-row behavior unchanged while the style source changes

## Testing

- [x] **6.** Add focused regression and unit coverage for tag validation and fallback behavior
  - [x] **6.1** Test valid and invalid tag parsing, including malformed separators and uppercase input
  - [x] **6.2** Test theme lookup for exact tag matches, parent fallback, and default-style fallback
  - [x] **6.3** Test that built-in syntax definitions and built-in themes load successfully after the migration
  - [x] **6.4** Test representative markup tags, including interior text refinements such as `markup.quote.text` or `markup.list.text`

- [x] **7.** Run verification and fix regressions
  - [x] **7.1** Run `cargo check` and fix any compile errors or warnings
  - [x] **7.2** Run the relevant test subset for syntax loading, theme loading, and rendering
  - [x] **7.3** Run the full test suite before marking the work complete

## Completion Summary

| Area | Tasks | Status |
| --- | --- | --- |
| Backend | 4 | Complete |
| Rendering | 1 | Complete |
| Testing | 2 | Complete |
| Total | 7 | Complete |
