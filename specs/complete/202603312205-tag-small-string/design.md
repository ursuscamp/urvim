# Tag Small String - Technical Design

## Architecture Overview

This change keeps `Tag` as the canonical owned syntax-tag type, but swaps its internal storage from `String` to a small-string implementation optimized for short, frequently cloned values.

`Tag` remains the single public abstraction used by syntax loading, theme lookup, buffer syntax spans, and tests. The only structural change is the backing storage type inside `Tag`.

`SmolStr` is the recommended dependency for this refactor because it provides:

- `O(1)` cloning
- small inline storage for short strings
- an owned, immutable string API that still exposes `&str`

`compact_str` was considered, but it is a weaker fit for this specific goal because clone cost still scales with string length. Since `Tag` values are validated identifiers and are often duplicated, clone cost is the primary optimization target.

## Interface Design

### Tag type

Keep the public `Tag` API stable while changing the internal field type.

```rust
pub struct Tag(SmolStr);

impl Tag {
    pub fn parse(value: &str) -> Result<Self, TagError>;
    pub fn as_str(&self) -> &str;
    pub fn parent_chain(&self) -> TagParents<'_>;
}
```

Behavior:

- `Tag::parse()` continues to validate the input before storing it
- `Tag::as_str()` continues to expose the canonical tag text as a borrowed string slice
- `Tag` continues to derive `Clone`, `Eq`, `Ord`, `Hash`, and `PartialEq`
- clone operations become cheap because the backing type is clone-optimized

### Tag error type

`TagError` remains unchanged at the API level.

```rust
pub struct TagError {
    input: String,
}
```

Behavior:

- invalid tag text is still reported using the original input string
- error formatting remains `invalid tag: <value>`

## Data Models

### Tag storage model

`Tag` stores a validated, canonical lowercase tag string in `SmolStr`.

Constraints:

- the stored text is identical to the accepted input after trimming
- validation still happens before allocation is committed to the `Tag`
- equality, ordering, and hashing continue to operate on the canonical string contents

### Small-string selection model

The chosen small-string type should satisfy these properties:

- cheap cloning for `Tag`
- immutable owned storage
- `&str` access without conversion
- support for the current Rust edition and the project dependency set

`SmolStr` satisfies those requirements and has the smallest design impact on the existing code.

## Key Components

### `src/theme/tag.rs`

Responsibilities:

- validate tag text
- store the canonical representation
- expose borrowed string access
- provide parent-chain iteration

Required implementation changes:

- replace the internal `String` field with `SmolStr`
- update construction in `Tag::parse()` to allocate the new storage type
- keep iterator and validation logic unchanged unless a type signature requires a small adjustment

### Tag consumers

Consumers should not need behavioral changes.

Responsibilities:

- continue accepting owned `Tag` values
- continue borrowing `&Tag` when lookup APIs only need read access
- continue using `Tag` as a map key, set key, or span field

The only observable difference should be reduced clone overhead.

## User Interaction

No user-facing editor behavior changes are expected.

Potentially observable effects:

- less allocation churn in theme and syntax code paths
- improved responsiveness in repeated tag-heavy operations

## External Dependencies

### `smol_str`

Add `smol_str` as the new backing string dependency for `Tag`.

Expected properties:

- `SmolStr` provides cheap cloning
- short tags stay inline
- longer tags remain valid owned strings

### Existing Rust ecosystem dependencies

No other dependency changes are required for the `Tag` refactor itself.

## Error Handling

Validation failure behavior remains unchanged.

- malformed tag text still returns `TagError`
- the original trimmed input remains attached to the error
- callers continue to handle parse failures at load time, not render time

No new runtime failure modes are introduced beyond dependency resolution and normal compile-time type checking.

## Security

This change does not introduce authentication, authorization, or secret-handling concerns.

Tag parsing continues to reject malformed input and does not execute or interpret tag text as code.

## Configuration

No configuration changes are required.

Tag storage is an internal implementation detail and does not affect user-facing settings.

## Component Interactions

```text
Syntax loader parses tag text
  -> Tag::parse validates and stores canonical text in SmolStr
  -> syntax spans and theme mappings carry Tag by value
  -> clones of Tag become cheap when values are copied through collections or style resolution
  -> parent-chain lookup continues to borrow &str views from the stored tag
```

The surrounding syntax and theme code should remain unchanged except for any compile-time adjustments caused by the storage type swap.

## Platform Considerations

`SmolStr` is a Rust library-level change and should behave consistently across supported platforms.

Because `Tag` strings are already ASCII-only by validation, the storage change does not alter Unicode handling, locale behavior, or platform-specific parsing rules.
