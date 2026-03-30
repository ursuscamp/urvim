# Shell Family Grammars - Implementation Tasks

## Overview
Promote the shell family from one partial portable shell grammar plus metadata-only dialect stubs into distinct built-in grammars for `shell`, `bash`, `zsh`, and `fish`, with fixture-driven regression coverage for each supported dialect.

## Backend
- [x] **1.** Rework the shared shell baseline.
  - [x] **1.1** Review `src/syntax/builtins/shell.toml` and keep the portable shell rules that should remain common across the family.
  - [x] **1.2** Adjust shared shell rules only where they need to support being extended by the dialect grammars.
  - [x] **1.3** Refresh `fixtures/syntax/shell.sh` so it still exercises the shared portable shell constructs after the family split.
- [x] **2.** Build a real Bash grammar.
  - [x] **2.1** Populate `src/syntax/builtins/bash.toml` with Bash metadata, filename and shebang detection, and Bash-specific lexical rules.
  - [x] **2.2** Add or extend `fixtures/syntax/bash.bash` with Bash-specific constructs such as `[[ ... ]]`, `(( ... ))`, arrays, `$'...'`, and heredoc-style forms.
  - [x] **2.3** Add buffer tests that confirm Bash files resolve to the Bash grammar and highlight Bash-specific token families.
- [x] **3.** Build a real Zsh grammar.
  - [x] **3.1** Populate `src/syntax/builtins/zsh.toml` with Zsh metadata, filename and shebang detection, and Zsh-specific lexical rules.
  - [x] **3.2** Add or extend `fixtures/syntax/zsh.zsh` with Zsh-specific constructs such as parameter expansion and globbing forms.
  - [x] **3.3** Add buffer tests that confirm Zsh files resolve to the Zsh grammar and highlight Zsh-specific token families.
- [x] **4.** Build a real Fish grammar.
  - [x] **4.1** Populate `src/syntax/builtins/fish.toml` with Fish metadata, filename and shebang detection, and Fish-specific lexical rules.
  - [x] **4.2** Add or extend `fixtures/syntax/fish.fish` with Fish-specific constructs such as Fish variable expansion, command substitution, and keywords.
  - [x] **4.3** Add buffer tests that confirm Fish files resolve to the Fish grammar and highlight Fish-specific token families.
- [x] **5.** Validate the shell-family split.
  - [x] **5.1** Run the focused syntax regression tests for shell, Bash, Zsh, and Fish fixtures.
  - [x] **5.2** Run `cargo check` and fix any build errors or warnings introduced by the grammar updates.

## Testing
- [x] **6.** Confirm the shell-family split is covered by regression tests.
  - [x] **6.1** Verify each of the four shell-family grammars has at least one fixture case that would have been under-highlighted before this change.
  - [x] **6.2** Verify the tests still pass for the existing shell highlighting behavior that should remain shared across the family.

## Completion Summary

| Area | Tasks | Status |
| --- | --- | --- |
| Backend | 5 | Complete |
| Testing | 1 | Complete |
| Total | 6 | Complete |
