# 202604032319: HTML attribute-string entities should be highlighted inside quoted values

## Summary
The built-in HTML syntax currently does not clearly highlight escaped character entities such as `&amp;` when they appear inside attribute strings. The visible issue is in quoted attribute values, where entity text should remain part of the string while still receiving entity-style highlighting.

## Severity: Low

This is a user-visible syntax highlighting regression, but it does not affect buffer contents, editing behavior, or persistence.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Fixture: `src/buffer/tests/syntax/fixtures/html.html`
- Relevant syntax definition: `src/syntax/builtins/html.toml`
- Existing syntax test: `src/buffer/tests/syntax/html.rs`

## Reproduction Steps

1. Open `src/buffer/tests/syntax/fixtures/html.html` with HTML syntax highlighting enabled.
2. Inspect the `<img>` line that contains `alt="Hi &amp; bye"`.
3. Notice that the attribute value is treated as a single quoted string, but the escaped entity text inside the string does not receive distinct entity highlighting.
4. Compare that with the standalone entity on the `<p>&amp;</p>` line, which is already recognized by the HTML entity rule.

## Expected Behavior

- Quoted attribute values should remain styled as strings.
- Escaped character entities inside attribute strings, such as `&amp;`, should also be highlighted as entities rather than blending into plain string text.
- The standalone entity and the attribute-string entity should use the same entity styling.

## Actual Behavior

- The standalone `&amp;` in element content is highlighted as an entity.
- The same kind of entity inside a quoted attribute value is not highlighted with the same entity style, even though it is part of valid HTML text.

## Impact

- Attribute values with encoded characters are harder to scan, especially in markup-heavy files.
- HTML highlighting looks inconsistent between element text and attribute strings.
- The editor loses a useful visual cue for encoded content inside attributes.

## Root Cause

The HTML grammar treats quoted attribute values as a single string region, while the entity rule is defined separately at the top level. That means entity-like text inside an attribute string does not get its own dedicated highlighting path and falls back to plain string styling.

## Solution Approach

- Update the HTML syntax definition so quoted attribute values can still recognize entity sequences inside the string body.
- Keep the attribute value styled as a string overall, but let valid entity text inside that region resolve to the entity tag.
- Extend the HTML syntax regression coverage so the existing `alt="Hi &amp; bye"` case explicitly asserts entity styling inside the attribute string.

## Code Changes

- `src/syntax/builtins/html.toml`
  - Split quoted attribute values into explicit string-body handling that can recognize HTML entities inside the attribute text.
- `src/buffer/tests/syntax/html.rs`
  - Add or tighten assertions so the `<img>` attribute string confirms `&amp;` receives entity styling.
- `src/buffer/tests/syntax/fixtures/html.html`
  - Keep the existing attribute-value example, or adjust it to make the entity-in-string case more explicit if needed.

## Edge Cases

- Plain attribute values without entities should continue to render as strings.
- Standalone entities outside attribute values should keep their current highlighting.
- Attribute values using either single or double quotes should behave the same way.
- Unterminated or malformed attribute strings should not crash highlighting or leak context into the rest of the document.
