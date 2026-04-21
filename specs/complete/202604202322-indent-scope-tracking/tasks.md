# Indent Scope Tracking - Implementation Tasks

## Overview
Implement buffer-cached indent scope tracking that is invalidated and rebuilt alongside syntax cache updates, with normalized visual indentation matching, nested scopes, EOF closure support, and both per-scope and per-line lookup APIs.

## Backend
- [x] **1.** Add indent scope cache data models and buffer storage.
  - [x] **1.1** Introduce `IndentScope` and scope-id model with documented invariants.
  - [x] **1.2** Introduce `IndentScopeCache` with `scopes` and `line_to_scopes` structures.
  - [x] **1.3** Add buffer fields/state to store scope cache and stale/generation metadata aligned with syntax cache lifecycle.

- [x] **2.** Wire cache lifecycle to syntax invalidation and rebuild.
  - [x] **2.1** Update syntax-cache invalidation path to also invalidate indent scope cache.
  - [x] **2.2** Update syntax rebuild coordinator to recompute and commit indent scopes in the same rebuild pass. (depends on: 1.2)
  - [x] **2.3** Ensure highlight cache and indent scope cache commit atomically for a single generation. (depends on: 2.2)

- [x] **3.** Implement normalized visual indent width logic.
  - [x] **3.1** Add helper that computes leading indentation width with tab expansion.
  - [x] **3.2** Resolve tab width from configuration when available.
  - [x] **3.3** Apply fallback tab width `4` when configuration is unavailable. (depends on: 3.2)

- [x] **4.** Implement indent scope builder algorithm.
  - [x] **4.1** Build scopes from consecutive boundary lines with equal normalized indentation.
  - [x] **4.2** Support nested scopes and preserve outer-to-inner ordering in per-line lookup.
  - [x] **4.3** Allow EOF closure for open scopes when valid interior lines exist.
  - [x] **4.4** Enforce interior-line rules: whitespace-only lines count as non-empty; zero-length lines are empty.

- [x] **5.** Expose public read APIs for scope consumers.
  - [x] **5.1** Add API for per-scope records (`start_line`, `end_line`, `indent_width`).
  - [x] **5.2** Add API for per-line containing-scope lookup.
  - [x] **5.3** Add rustdoc comments for new public modules/types/methods.

## Testing
- [x] **6.** Add unit tests for indent normalization.
  - [x] **6.1** Verify mixed tabs/spaces normalize to equal visual width where expected.
  - [x] **6.2** Verify fallback tab width `4` behavior when config is unavailable.

- [x] **7.** Add unit tests for scope construction.
  - [x] **7.1** Verify simple closed scope detection.
  - [x] **7.2** Verify nested scope detection and ordering.
  - [x] **7.3** Verify EOF-closing scope recording.
  - [x] **7.4** Verify interior-line rules for whitespace-only and strictly empty lines.

- [x] **8.** Add integration/regression tests for cache lifecycle.
  - [x] **8.1** Verify syntax invalidation also invalidates indent scope cache.
  - [x] **8.2** Verify syntax rebuild refreshes highlight and indent caches together.
  - [x] **8.3** Verify scope APIs return data consistent with rebuilt cache generation.

## Documentation
- [x] **9.** Document terminology and behavior for future feature consumers.
  - [x] **9.1** Add glossary entry updates for indent scope terms.
  - [x] **9.2** Add concise inline comments for non-obvious scope-builder logic.

## Completion Summary
| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Backend | 5 | 5 | Done |
| Testing | 3 | 3 | Done |
| Documentation | 1 | 1 | Done |
| Total | 9 | 9 | Done |
