# Case Switching Operators - Technical Design

## Architecture Overview
urvim will treat case switching as a small family of edit operations that share the same target-resolution pipeline as the existing editing operators.

The implementation will split cleanly into three layers:
- key handling, which recognizes `gu`, `gU`, and `g~`
- target resolution, which determines the active motion target or visual selection
- text transformation, which rewrites the resolved text using Unicode-aware casing rules

The same transform path will be used for:
- operator-pending invocations from normal mode
- active characterwise visual selections
- active linewise visual selections

This keeps the feature consistent with the current operator model and avoids separate code paths for normal and visual edits.

## Interface Design
The editor will introduce an internal case-transform kind that identifies the requested casing action.

Suggested interface additions:
- `CaseTransformKind`: a small enum with `Lower`, `Upper`, and `Toggle`
- `Operator`: extended to carry a case-transform variant, or an equivalent internal representation for case ops
- `ActionKind`: extended with actions that let normal mode and visual mode request the same case-transform behavior

The shared editing surface should expose a helper that applies one case transform to a resolved range:

```rust
fn apply_case_transform(&mut self, range: TextObjectRange, kind: CaseTransformKind) -> ActionResult
```

Visual-mode entry points should call the same helper after they resolve the active selection range. Linewise visual mode should route through the linewise equivalent of that range resolution.

The interface should not introduce any new configuration knobs.

## Data Models
### `CaseTransformKind`
Represents the requested transformation:
- `Lower` lowers the target text
- `Upper` uppercases the target text
- `Toggle` inverts the case of each character where a one-to-one case partner exists

### Range Inputs
Case switching operates on the same resolved text-range models already used by urvim:
- `TextObjectRange` for characterwise selections and motions
- line start plus count for linewise visual selections

The feature does not need to persist any new editor state.

## Key Components
### Normal Mode Key Handling
The normal-mode parser will recognize the three `g`-prefixed operator sequences and emit the corresponding case-transform action.

Responsibility:
- distinguish `gu`, `gU`, and `g~`
- preserve existing count parsing and register-prefix behavior where the operator model already supports it
- keep cancellation behavior consistent with the existing `g`-prefixed command handling

### Visual Mode Dispatch
Visual mode will map the same three keys to the same transform kind, but instead of waiting for a motion it will use the active selection.

Responsibility:
- resolve the current visual range
- apply the case transform to the selection
- exit visual mode in the same way other visual edits do

### Shared Transform Helper
The window/edit layer should own a single helper that:
- reads the target text
- transforms it in memory
- replaces the original range with the transformed text
- restores the cursor to the start of the transformed region, consistent with other visual edits

This helper should treat the edit as one logical operation so undo and repeat behavior remain coherent.

### Text Transformation Logic
The transform logic should operate per Unicode scalar value and use the standard Rust casing APIs:
- `to_lowercase()` for lowercase conversion
- `to_uppercase()` for uppercase conversion

`g~` should be defined as a per-character case inversion:
- lowercase characters become uppercase
- uppercase characters become lowercase
- characters without a clear case partner remain unchanged

This matches the user expectation of a Vim-like toggle while still respecting Unicode-aware casing where Rust exposes it.

## User Interaction
### Normal Mode
- `gu{motion}` lowercases the resolved target
- `gU{motion}` uppercases the resolved target
- `g~{motion}` toggles the resolved target case

These should work with the same motions and text objects that other operators already accept.

### Visual Mode
- select text in characterwise or linewise visual mode
- press `gu`, `gU`, or `g~`
- urvim transforms the selected text in place and exits visual mode

The selection itself should be the source of truth; no extra motion is needed.

### Unicode Cases
Unicode casing may expand or shrink the byte length of the text:
- `ß` uppercases to multiple characters
- some characters have no meaningful uppercase/lowercase swap

The editor should preserve the full transformed text, even when the result differs in length from the original.

## External Dependencies
The design relies on:
- Rust standard library casing iterators and Unicode character classification
- the existing buffer editing primitives for text extraction and replacement
- the current normal-mode and visual-mode input pipelines

No third-party crates are required for the feature itself.

## Error Handling
Expected failure cases should behave like existing no-op or cancellation paths:
- invalid operator sequences are rejected during key parsing
- empty or missing operator targets do nothing
- missing or invalid visual selections do nothing
- a canceled operator-pending sequence leaves the buffer unchanged

If the target text cannot be resolved, the editor should not partially transform anything.

Because Unicode casing can expand the output, the implementation should avoid byte-length assumptions when reconstructing the buffer. The transformation must be performed on the full string payload before replacement.

## Security
The feature does not introduce authentication, authorization, or secret-handling concerns.

The only input surface is editor text already present in the buffer, so the main safety requirement is to keep the transformation bounded to the resolved selection or motion target.

## Configuration
No configuration changes are required.

The new operators should be available by default wherever the current keymaps already expose operator and visual edit behavior.

## Component Interactions
1. Key input arrives in normal mode or visual mode.
2. The mode layer resolves `gu`, `gU`, or `g~` into a case-transform request.
3. The window layer resolves the target range from either the pending operator motion or the active visual selection.
4. The transform helper reads the selected text, converts it, and replaces the original range.
5. The buffer invalidates affected syntax state and the cursor returns to the transformed region start.

This flow keeps the feature aligned with the current operator architecture and makes it easy to test from both operator-pending and visual-selection entry points.

## Platform Considerations
Unicode casing behavior is defined by Rust's standard library and therefore follows the toolchain's Unicode tables on each platform.

The resulting behavior should be stable across operating systems, but exact mappings can vary if the Rust toolchain's Unicode data changes in a future release.
