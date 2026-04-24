# Floating Notification Banners

## Summary
urvim's top-right notifications will be upgraded from plain text banners into bordered floating windows. Each notification will use a border style that matches its severity level, and long messages will wrap inside the notification window instead of clipping to a single line. All built-in themes will provide level-specific notification styling, and temporary debug keybindings will be added so each notification level can be tested quickly.

## Problem Statement
The current notification presentation is functional but visually flat. Notifications appear as plain text in the top-right corner, which makes them harder to notice and less polished than the rest of the editor UI. Built-in themes do not currently define notification-level styling explicitly, so notification appearance is inconsistent across themes. There is also no convenient way to trigger sample notifications of each level during development to validate the visual treatment.

## User Stories
- As an urvim user, I want notifications to appear in a bordered floating window, so that they are easier to notice and more visually consistent with the rest of the UI.
- As an urvim user, I want notification border colors to reflect the notification level, so that I can recognize severity at a glance.
- As an urvim user, I want long notification messages to wrap inside the notification window, so that I can read them without losing content.
- As an urvim contributor, I want built-in themes to include notification-level styling, so that the notification appearance is intentional across bundled themes.
- As an urvim contributor, I want temporary keybindings that spawn sample notifications of each level, so that I can verify notification styling quickly during development.

## Functional Requirements
- [ ] **REQ-001**: Notifications shall continue to appear in the top-right region of the editor viewport.
- [ ] **REQ-002**: The active notification shall render inside a bordered floating window rather than as a plain text banner.
- [ ] **REQ-003**: The notification window border shall use a style derived from the notification's level.
- [ ] **REQ-004**: The notification window content shall wrap across multiple lines when the message exceeds the available width.
- [ ] **REQ-005**: Notification rendering shall preserve the existing queue order and sequential display behavior.
- [ ] **REQ-006**: Notification rendering shall remain non-focusable and shall not steal input focus from the editor.
- [ ] **REQ-007**: All built-in themes shall define styling for the existing notification levels: info, warn, and error.
- [ ] **REQ-008**: Notification styling shall remain safe when a theme omits one or more notification level styles, by falling back to a reasonable default.
- [ ] **REQ-009**: Temporary developer-facing keybindings shall enqueue a generic notification for each existing notification level.
- [ ] **REQ-010**: The temporary notification test keybindings shall use unused keys and shall not conflict with normal editing commands.

## Non-Functional Requirements
- **Usability**: Notification presentation should be more polished and immediately readable than the current flat text banner.
- **Compatibility**: The notification renderer shall work with all bundled themes and shall continue to function with custom themes that have not yet added explicit notification styles.
- **Reliability**: If a screen is too small or a style is unavailable, notification rendering shall fail gracefully rather than panic or corrupt the layout.
- **Performance**: Wrapping and border rendering shall remain lightweight enough to run on every frame without noticeable overhead.

## Acceptance Criteria
- [ ] **AC-001**: A visible notification appears in the top-right corner inside a bordered floating window.
- [ ] **AC-002**: Info, warn, and error notifications each render with a distinct border color or border style.
- [ ] **AC-003**: A long notification message wraps onto multiple lines inside the notification window.
- [ ] **AC-004**: Built-in themes render notification levels with intentional styles rather than relying only on fallback defaults.
- [ ] **AC-005**: Pressing each temporary debug key triggers a generic notification of the corresponding level.
- [ ] **AC-006**: Existing editor behavior continues to function normally while notifications are visible.

## Out of Scope
- Persistent notification history or scrolling history UI.
- Mouse-driven notification interaction or dismissal.
- Changing the existing notification queue semantics or TTL policy.
- Adding new notification levels.
- Permanent developer hotkeys or user-configurable notification keybinding settings.

## Assumptions
- The existing notification levels remain **Info**, **Warn**, and **Error**.
- The current notification queue and redraw behavior remain in place.
- The temporary test keybindings are only intended for local development and validation.
- Existing terminal border rendering capabilities can be reused for the notification frame.

## Dependencies
- Existing notification queue and rendering code in `src/notification`.
- Existing theme loading and built-in theme TOML files in `src/theme/builtin`.
- Existing terminal border glyph and style handling.
- Existing input dispatch and keybinding handling paths.
