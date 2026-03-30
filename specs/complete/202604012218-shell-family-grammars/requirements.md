# Shell Family Grammars

## Summary
urvim should promote the shell-family built-ins from a single partial shell grammar plus metadata-only aliases into distinct, real grammars for `shell`, `bash`, `zsh`, and `fish`, so that each common shell dialect is highlighted in a way that matches its lexical structure and filetype mappings.

## Problem Statement
The current shell-family coverage is uneven. `shell` has a partial grammar, while `bash`, `zsh`, and `fish` are only metadata shells or minimal stubs. That means files opened as Bash, Zsh, or Fish do not receive dialect-appropriate highlighting, and filename or shebang-based detection can point users at a syntax entry that does not yet describe the language they are editing.

This leaves common shell files looking unfinished in everyday use. It also makes the syntax catalog misleading, because the editor exposes shell-family filetypes that do not yet behave like first-class grammars.

## User Stories
- As a user editing portable shell scripts, I want `sh` files to keep their current shell highlighting, so that common comments, strings, expansions, substitutions, and heredoc-like regions stay readable.
- As a Bash user, I want `.bash` files and Bash shebangs to receive Bash-specific highlighting, so that dialect features such as `[[ ... ]]`, `(( ... ))`, arrays, `$'...'`, and Bash heredoc behavior are visually distinct.
- As a Zsh user, I want `.zsh` files and Zsh shebangs to receive Zsh-specific highlighting, so that Zsh-specific parameter expansion and globbing forms are not lost in plain shell highlighting.
- As a Fish user, I want `.fish` files and Fish shebangs to receive Fish-specific highlighting, so that Fish syntax differences from POSIX shell are recognized instead of being rendered as generic shell text.
- As a maintainer, I want the shell family to be represented by separate built-in grammars, so that future grammar improvements can be made per dialect without overloading one shared file.

## Functional Requirements
- [ ] **REQ-001**: The editor shall continue to provide a `shell` built-in grammar for portable POSIX-style shell text, including the current shell-family lexical coverage that is shared across common scripts.
- [ ] **REQ-002**: The editor shall provide a first-class `bash` built-in grammar with canonical metadata, filename mappings, and shebang detection for Bash scripts.
- [ ] **REQ-003**: The editor shall provide a first-class `zsh` built-in grammar with canonical metadata, filename mappings, and shebang detection for Zsh scripts.
- [ ] **REQ-004**: The editor shall provide a first-class `fish` built-in grammar with canonical metadata, filename mappings, and shebang detection for Fish scripts.
- [ ] **REQ-005**: Bash highlighting shall distinguish Bash-specific lexical forms that are not adequately represented by the portable shell grammar, including Bash test and arithmetic forms, Bash-style arrays, and Bash-specific quoted literal forms where they are visibly distinguishable.
- [ ] **REQ-006**: Zsh highlighting shall distinguish Zsh-specific lexical forms that are not adequately represented by the portable shell grammar, including Zsh parameter expansion and globbing forms where they are visibly distinguishable.
- [ ] **REQ-007**: Fish highlighting shall distinguish Fish-specific lexical forms that are not adequately represented by the portable shell grammar, including Fish variable expansion, command substitution, and keyword forms where they are visibly distinguishable.
- [ ] **REQ-008**: The shell-family grammars shall keep multiline strings, command substitutions, and heredoc-style regions stable across line boundaries where the dialect supports those forms.
- [ ] **REQ-009**: The shell-family grammars shall preserve the current shell grammar behavior for portable constructs while allowing dialect-specific grammars to refine the unsupported forms in their own files.
- [ ] **REQ-010**: The shell-family grammars shall not mislead users by leaving `.bash`, `.zsh`, and `.fish` files mapped to metadata-only stubs when the editor presents those filetypes as supported syntaxes.
- [ ] **REQ-011**: Regression fixtures shall exist for `shell`, `bash`, `zsh`, and `fish` and shall exercise the major lexical forms that distinguish each grammar.
- [ ] **REQ-012**: Buffer-level syntax tests shall verify that each shell-family fixture resolves to the intended grammar and highlights the intended lexical families.

## Non-Functional Requirements
- [ ] **NFR-001**: The shell-family grammars shall remain responsive during normal editing and scrolling.
- [ ] **NFR-002**: The shell-family grammars shall remain compatible with the existing regex-and-region syntax engine.
- [ ] **NFR-003**: The shell-family grammars shall remain maintainable by keeping shared portable rules in `shell` and dialect-specific rules in the respective dialect grammars.
- [ ] **NFR-004**: The shell-family grammars shall be covered by regression fixtures so future edits can be validated quickly.

## Acceptance Criteria
- [ ] **AC-001**: `.sh`, `.bash`, `.zsh`, and `.fish` files resolve to distinct shell-family grammars rather than metadata-only placeholders.
- [ ] **AC-002**: Bash fixtures show visible highlighting for Bash-specific lexical forms such as `[[ ... ]]`, `(( ... ))`, arrays, and Bash-specific quoted literals.
- [ ] **AC-003**: Zsh fixtures show visible highlighting for Zsh-specific lexical forms such as parameter expansion and globbing forms.
- [ ] **AC-004**: Fish fixtures show visible highlighting for Fish-specific lexical forms such as Fish expansion and command forms.
- [ ] **AC-005**: Portable shell constructs continue to highlight in the shared `shell` grammar without regressing current shell behavior.
- [ ] **AC-006**: Regression coverage exists for all four shell-family grammars and includes at least one case per dialect that would have remained under-highlighted before this change.

## Out of Scope
- Replacing the regex-and-region syntax engine with a parser-backed shell highlighter.
- Full semantic parsing of Bash, Zsh, or Fish grammar edge cases.
- Bash, Zsh, or Fish shell execution semantics beyond lexical highlighting.
- Adding new configuration options for selecting shell dialects manually.
- Changing non-shell language grammars as part of this work.

## Assumptions
- The existing shell-family metadata entries are already the correct starting point for filename and shebang detection.
- The current syntax engine can express the additional dialect-specific lexical forms through regex and delimited regions.
- Shared shell behavior should remain in `shell` so the dialect grammars can focus on language-specific differences instead of duplicating every common rule.
- Regression fixtures can capture the visible distinctions between the shell family grammars without needing executable shell semantics.

## Dependencies
- Existing built-in syntax definitions for `shell`, `bash`, `zsh`, and `fish`.
- Existing built-in syntax loader and filetype resolution.
- Existing buffer syntax regression test harness.
- Existing fixture loading path for syntax examples.
