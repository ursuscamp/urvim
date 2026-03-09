---
name: spec-implementer
description: Read and implement specs from specs/in-progress
---

# Spec Implementer Skill

This skill reads and implements specs from the in-progress queue.

## When to Use

Use this skill when asked to implement a spec from `specs/in-progress/`.

## Process

### 1. Find Next Spec

List all files in `specs/in-progress/` sorted by modification time (oldest first). Pick the oldest file as the next spec to implement.

### 2. Read and Analyze Spec

Read the spec file thoroughly. Understand:
- The feature/change being implemented
- Goals and non-goals
- Architecture and design details
- Edge cases to handle
- Testing requirements

### 3. Implementation

Implement the feature according to the spec. Follow project guidelines:
- Write unit tests
- Use `cargo check` to verify builds
- Create documentation comments for public modules/types/methods
- Log to debug.log for normal application logging

### 4. Confirm Completion

Ask the user to confirm the implementation is complete and meets their expectations.

### 5. Move Spec to Done

If confirmed complete, move the spec file from `specs/in-progress/` to `specs/done/`.

## Spec Location

- In-progress: `specs/in-progress/`
- Completed: `specs/done/`
