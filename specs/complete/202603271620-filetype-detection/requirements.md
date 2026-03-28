# Filetype Detection

## Summary

Add filetype detection to buffers so urvim can classify common editor-friendly filetypes from filenames and shebang lines, expose that classification through buffer state, and display the active filetype in the status line.

## Problem Statement

Buffers currently know about file names, but the editor does not classify the underlying filetype. That leaves the status bar without a useful filetype indicator and makes it harder to add future syntax- or filetype-aware behavior in a consistent way. The editor needs a small, explicit filetype model that covers common code-editor filetypes and can infer a sensible result from the filename or shebang line when the filename alone is not enough.

## User Stories

- As a user, I want the status bar to show the current filetype, so that I can confirm how urvim has classified the active buffer.
- As a user, I want urvim to detect filetype from common filenames and extensions, so that files open with a sensible classification without extra setup.
- As a user, I want urvim to detect filetype from a shebang line when a filename is not informative, so that script buffers still display the right type.
- As a maintainer, I want filetype classification to live on the buffer model, so that other parts of the editor can reuse the same source of truth.

## Functional Requirements

- [ ] **REQ-001**: The editor shall provide a public filetype enum that covers common and less common editor-friendly programming languages supported by urvim.
- [ ] **REQ-002**: Each buffer shall expose its current filetype through a read-only buffer API.
- [ ] **REQ-003**: Filetype detection shall use the buffer filename when one is available.
- [ ] **REQ-004**: Filename-based detection shall recognize common extensions and common extensionless editor filenames for both mainstream and less common programming languages supported by the enum.
- [ ] **REQ-005**: When filename-based detection is inconclusive, the editor shall inspect the first line for a shebang and classify interpreter-based filetypes supported by the enum.
- [ ] **REQ-006**: Shebang detection shall ignore the interpreter arguments and `/usr/bin/env` wrapper when determining filetype.
- [ ] **REQ-007**: Unknown or unsupported filenames and shebangs shall fall back to a stable default filetype instead of failing.
- [ ] **REQ-008**: Buffer filetype shall stay in sync with buffer metadata and relevant text content so the reported filetype reflects the current buffer state.
- [ ] **REQ-009**: The status bar shall display the active buffer filetype alongside the existing buffer metadata.
- [ ] **REQ-010**: The status bar shall continue to render a readable fallback label for unnamed buffers.
- [ ] **REQ-011**: Filetype detection shall not mutate buffer contents or perform any filesystem actions beyond reading the buffer's existing filename and text.

## Non-Functional Requirements

- **Usability**: The filetype label in the status bar shall be short, readable, and consistent across buffers.
- **Predictability**: Detection should follow a deterministic precedence order so the same buffer always resolves to the same filetype for the same filename and shebang line.
- **Maintainability**: Filetype classification should be centralized so future filetype additions only require extending one detection surface.
- **Compatibility**: Existing buffer loading, rendering, and editing flows shall continue to behave as they do today when filetype detection cannot determine anything more specific than the fallback.

## Acceptance Criteria

- [ ] **AC-001**: Opening a Rust source file shows a Rust filetype label in the status bar.
- [ ] **AC-002**: Opening a common script without a useful extension but with a Python shebang shows Python as the filetype.
- [ ] **AC-003**: Opening a file with an unrecognized name and no shebang shows the fallback filetype label instead of an error.
- [ ] **AC-004**: A buffer with no filename still renders a readable filetype label in the status bar.
- [ ] **AC-005**: Filetype detection is based on buffer data and does not require user configuration.
- [ ] **AC-006**: Existing cursor, mode, buffer name, and progress information still appears in the status bar after the filetype label is added.

## Out of Scope

- Syntax highlighting or token-based parsing
- Filetype-specific indentation, format-on-save, or linting behavior
- User-configurable filetype detection rules
- Project-local filetype overrides
- Language server integration

## Assumptions

- urvim will use a curated enum of common filetypes rather than attempting exhaustive editor/filetype coverage on day one.
- The fallback classification will be a readable general-purpose label suitable for plain text editing.
- Shebang detection only needs to cover the common interpreter forms used for editor-friendly scripts.
- Filetype detection can be recomputed when the buffer's relevant metadata or first line changes without requiring a full re-scan of the file.

## Dependencies

- Existing buffer filename and file loading behavior
- Existing status bar rendering and layout composition
- Existing global buffer access patterns
- Existing screen and footer rendering tests
