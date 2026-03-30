# Syntax Filetype Metadata

## Summary
Refactor urvim so filetype classification comes from syntax grammar metadata instead of a dedicated filetype enum. Syntax files become the single source of truth for canonical names, display labels, filename matching, and shebang matching.

## Problem Statement
urvim currently spreads filetype behavior across a Rust enum, hardcoded detection logic, syntax registry aliases, and UI display labels. That makes filetype-related changes expensive and easy to get out of sync. The editor needs a data-driven model where syntax grammar files describe the filetype they represent and the editor consumes that metadata everywhere it needs classification or display.

## User Stories
- As a user, I want urvim to recognize common filetypes from the active buffer’s filename or shebang, so that files open with the right syntax automatically.
- As a user, I want the filetype shown in the status bar to use the syntax’s display label, so that the UI stays readable and consistent.
- As a maintainer, I want filetype behavior to live in syntax grammar files, so that adding or changing a filetype does not require editing a separate enum.
- As a maintainer, I want the editor to keep working when no syntax metadata matches, so that unknown files still open safely.

## Functional Requirements
- [ ] **REQ-001**: Syntax grammar files shall declare filetype metadata under a `[metadata]` section.
- [ ] **REQ-002**: Each filetype metadata block shall define a canonical `name` that uniquely identifies the syntax.
- [ ] **REQ-003**: Each filetype metadata block shall define a `display_name` used for user-facing UI labels.
- [ ] **REQ-004**: Each filetype metadata block shall define a `filename` list of regular expressions matched against the buffer’s basename for filename-based classification.
- [ ] **REQ-005**: Each filetype metadata block shall define a `shebang` list of regular expressions matched against the buffer’s shebang line for shebang or magic-line classification.
- [ ] **REQ-006**: Filetype resolution shall be deterministic and shall prefer filename matches over shebang matches when both are available.
- [ ] **REQ-007**: Filetype behavior shall be driven from syntax metadata rather than a hardcoded Rust filetype enum.
- [ ] **REQ-008**: The editor shall continue to support common special filenames and interpreter shebang cases through syntax metadata, including full filenames such as `Dockerfile` and script interpreters such as Python, Bash, and PowerShell.
- [ ] **REQ-009**: When no syntax metadata matches, the editor shall fall back to a stable default syntax/filetype representation instead of erroring.
- [ ] **REQ-010**: The syntax loader shall validate filetype metadata and report invalid or conflicting definitions clearly.
- [ ] **REQ-011**: The syntax registry shall continue to resolve syntax definitions by canonical syntax name for highlighting and nested syntax references.
- [ ] **REQ-012**: Public UI surfaces that currently show a filetype label shall continue to show a readable label derived from the syntax metadata.
- [ ] **REQ-013**: Every filetype currently supported by the removed enum shall have a corresponding built-in syntax definition after the migration, even if the syntax initially contains only metadata and empty rule sets.

## Non-Functional Requirements
- **Compatibility**: Existing buffer loading, rendering, and syntax highlighting flows shall continue to work for supported filetypes after the enum is removed.
- **Usability**: The status bar label shall remain short, readable, and stable for the same syntax.
- **Maintainability**: Adding or changing filetypes shall require editing syntax grammar metadata only, without touching a separate filetype enum.
- **Reliability**: Unknown files, empty buffers, and invalid metadata shall fail safely to the fallback representation.

## Acceptance Criteria
- [ ] **AC-001**: Opening a Rust source file resolves to the Rust syntax metadata and shows the Rust display name in the status bar.
- [ ] **AC-002**: Opening a file such as `Dockerfile` or a script with a supported shebang resolves through syntax metadata without requiring a Rust enum variant.
- [ ] **AC-003**: Opening a file with no matching filename or shebang rule falls back to the default representation instead of producing an error.
- [ ] **AC-004**: The status bar continues to render the active buffer’s filetype label alongside the existing mode, buffer name, cursor position, and progress information.
- [ ] **AC-005**: Syntax definitions continue to resolve by canonical syntax name for highlighting and nested syntax references.
- [ ] **AC-006**: Every filetype previously represented in the enum has a corresponding built-in syntax definition after the migration.

## Out of Scope
- User-configurable filetype overrides
- Project-local syntax search path changes
- New syntax highlighting rules beyond the metadata refactor
- Changes to editor commands unrelated to filetype resolution

## Assumptions
- The fallback representation will remain a readable general-purpose label suitable for plain text editing.
- Syntax metadata regexes will use the repository’s existing Rust regex semantics.
- Existing syntax rule bodies remain valid and do not need behavioral changes beyond the metadata refactor.

## Dependencies
- Syntax loader and registry changes
- Buffer filetype resolution changes
- Status bar and layout label rendering
- Built-in syntax TOML file updates
