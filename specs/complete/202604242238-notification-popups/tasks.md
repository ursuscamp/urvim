# Floating Notification Banners - Implementation Tasks

## Overview
Upgrade the notification renderer into a bordered floating window, add level-specific styling to bundled themes, and add temporary debug keybindings for exercising each notification level. Preserve the existing notification queue, sequential display behavior, and top-right placement.

## Backend
- [x] **1.** Refactor notification rendering to draw a bordered floating window in the top-right corner.
  - [x] **1.1** Compute popup bounds from the active notification text, terminal size, and available top-right space.
  - [x] **1.2** Wrap notification text across multiple lines and size the popup around the wrapped content.
  - [x] **1.3** Reuse the existing border glyph strategy so the popup frame matches the current border mode.
  - [x] **1.4** Apply a level-specific border style while keeping the popup body readable.
- [x] **2.** Keep notification queue behavior unchanged while switching to the popup renderer.
  - [x] **2.1** Preserve FIFO display order for queued notifications.
  - [x] **2.2** Preserve redraw requests and expiration-driven advancement. (depends on: 2.1)
  - [x] **2.3** Add regression tests for wrapping, border placement, and sequential display.
- [x] **3.** Add level-specific notification styles to every built-in theme.
  - [x] **3.1** Update each built-in theme TOML with `ui.notification.info`, `ui.notification.warn`, and `ui.notification.error`.
  - [x] **3.2** Add theme loader regression tests to verify bundled themes expose notification styles.
  - [x] **3.3** Verify fallback styles still render safely when a custom theme omits notification styling.

## Frontend
- [x] **4.** Add temporary debug keybindings for notification testing.
  - [x] **4.1** Bind one unused key per notification level in the normal input path.
  - [x] **4.2** Route each binding to enqueue a generic notification for its level.
  - [x] **4.3** Ensure the temporary bindings do not conflict with existing editor commands or motions.
- [x] **5.** Confirm the notification popup works with the active layout and theme pipeline.
  - [x] **5.1** Verify the popup stays in the top-right without stealing focus.
  - [x] **5.2** Verify long messages wrap cleanly across multiple lines in common terminal sizes.
  - [x] **5.3** Verify built-in themes produce distinct info/warn/error border colors.

## Testing
- [x] **6.** Add or update automated tests for notification rendering and theme styling.
  - [x] **6.1** Cover popup rendering with info, warn, and error notifications.
  - [x] **6.2** Cover long-message wrapping and narrow-screen clamping behavior.
  - [x] **6.3** Cover the temporary debug bindings enqueueing the expected notification level.
- [x] **7.** Run project validation after the changes are in place.
  - [x] **7.1** Format the codebase.
  - [x] **7.2** Run `cargo check` and relevant test suites.

## Completion Summary

| Section | Total | Done | Status |
|---|---:|---:|---|
| Backend | 3 | 3 | Done |
| Frontend | 2 | 2 | Done |
| Testing | 2 | 2 | Done |
| Overall | 7 | 7 | Done |
