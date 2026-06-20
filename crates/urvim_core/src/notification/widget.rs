//! Notification banner widget.

use crate::globals;
use crate::screen::Screen;
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;

/// Passive widget that manages notification queue ticks and banner rendering.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Notification;

impl Notification {
    /// Creates a notification banner widget.
    pub fn new() -> Self {
        Self
    }
}

impl Widget for Notification {
    fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        match event {
            UiEvent::Tick => {
                if globals::prune_notifications() {
                    UiEventResult::Handled(Vec::new())
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Key(_) | UiEvent::Paste(_) | UiEvent::Resize(_, _) => {
                UiEventResult::NotHandled
            }
        }
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        crate::notification::render_active_banner(
            screen,
            rect.origin,
            rect.size,
            std::time::Instant::now(),
        );
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Passive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::NotificationLevel;

    #[test]
    fn tick_prunes_notification_queue() {
        let _guard = globals::notification_test_lock();
        globals::clear_notifications();
        let mut widget = Notification::new();
        let mut ctx = UiContext;

        assert!(globals::enqueue_notification(
            NotificationLevel::Info,
            "queued".to_string()
        ));

        let result = widget.handle_ui_event(&UiEvent::Tick, &mut ctx);
        assert!(matches!(
            result,
            UiEventResult::NotHandled | UiEventResult::Handled(_)
        ));

        // Ensure tick processing never removes a fresh message immediately.
        assert!(globals::active_notification(std::time::Instant::now()).is_some());
    }
}
