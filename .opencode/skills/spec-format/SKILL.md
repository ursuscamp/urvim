---
name: spec-format
description: Write implementation specs in the repository standard format
---

# Software Specification Skill

This skill guides the creation of software specification documents for the urvim project.

## When to Use

Use this skill when:
- Writing a new feature specification
- Creating a design document for a significant change
- Documenting a bug fix that requires architectural changes
- Planning any non-trivial implementation

## Specification Location

All specs in progress should be saved to: `specs/in-progress/`

## Filename Format

Use the format: `YYYYMMDDHHmmss-spec-name-slug.md`

Example: `20240315143000-vim-mode-refactor.md`

## Specification Template

```markdown
# Specification: [Feature Name]

## Overview
Brief description of what this feature does and why it's needed.

## Goals
- Goal 1
- Goal 2

## Non-Goals
- What this feature will NOT do

## Background
Context and motivation for this change.

## Detailed Design

### Architecture
High-level architecture changes.

### Data Structures
New or modified data structures.

### Algorithms
Key algorithms or approaches.

### API Design
If applicable, the public API surface.

## Edge Cases
Handle edge cases and error conditions.

## Testing Strategy
How to test this feature.

## Open Questions
Questions to be resolved during implementation.
```

## Process

1. Create spec in `specs/in-progress/` with proper filename
2. Implement the feature
3. Once feature is complete, move spec to `specs/completed/` (if that directory exists)
