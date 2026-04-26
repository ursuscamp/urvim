//! Notification queue state and TTL progression.

use super::model::{NotificationLevel, NotificationMessage, PendingNotification};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const NORMAL_TTL: Duration = Duration::from_secs(3);
const BACKLOG_TTL: Duration = Duration::from_secs(1);

/// In-memory notification queue state.
#[derive(Debug, Default)]
pub struct NotificationState {
    active: Option<NotificationMessage>,
    pending: VecDeque<PendingNotification>,
    redraw_requested: bool,
}

impl NotificationState {
    /// Creates an empty notification queue state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the active notification, if one is currently displayed.
    pub fn active(&self) -> Option<&NotificationMessage> {
        self.active.as_ref()
    }

    /// Returns the number of queued pending notifications.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Enqueues a notification message.
    ///
    /// Empty or whitespace-only messages are ignored.
    pub fn enqueue(&mut self, level: NotificationLevel, text: String, now: Instant) -> bool {
        if text.trim().is_empty() {
            return false;
        }

        if self.active.is_none() {
            self.active = Some(NotificationMessage::new(level, text, now, NORMAL_TTL));
            self.redraw_requested = true;
            return true;
        }

        let pending = PendingNotification {
            level,
            text,
            created_at: now,
            backlog_ttl: true,
        };

        self.pending.push_back(pending);
        self.redraw_requested = true;
        true
    }

    /// Prunes expired active notifications and advances the queue.
    pub fn prune_and_advance(&mut self, now: Instant) -> bool {
        let mut changed = false;

        while self
            .active
            .as_ref()
            .is_some_and(|active| active.expired(now))
        {
            changed = true;
            if let Some(next) = self.pending.pop_front() {
                let ttl = if next.backlog_ttl {
                    BACKLOG_TTL
                } else {
                    NORMAL_TTL
                };
                self.active = Some(NotificationMessage::new(next.level, next.text, now, ttl));
            } else {
                self.active = None;
            }
        }

        if changed {
            self.redraw_requested = true;
        }

        changed
    }

    /// Clears active and pending notifications.
    pub fn clear(&mut self) {
        self.active = None;
        self.pending.clear();
        self.redraw_requested = false;
    }

    /// Requests a redraw for the notification surface.
    pub fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    /// Returns and clears the redraw-requested flag.
    pub fn take_redraw_requested(&mut self) -> bool {
        let requested = self.redraw_requested;
        self.redraw_requested = false;
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_sets_active_message() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "Saved".to_string(), now));
        let active = state.active().expect("active notification");
        assert_eq!(active.text, "Saved");
        assert_eq!(active.level, NotificationLevel::Info);
        assert!(active.expires_at.duration_since(active.created_at) >= NORMAL_TTL);
    }

    #[test]
    fn enqueue_multiple_messages_keeps_fifo_pending() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Error, "three".to_string(), now));

        assert_eq!(state.pending_len(), 2);
        assert_eq!(
            state.active().map(|message| message.text.as_str()),
            Some("one")
        );
    }

    #[test]
    fn prune_advances_queue_in_order() {
        let mut state = NotificationState::new();
        let now = Instant::now();
        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));

        let first_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(first_expiry));
        assert_eq!(
            state.active().map(|message| message.text.as_str()),
            Some("two")
        );

        let second_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(second_expiry));
        assert!(state.active().is_none());
    }

    #[test]
    fn backlog_messages_use_shorter_ttl() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Error, "three".to_string(), now));

        let first_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(first_expiry));

        let second = state.active().expect("active");
        assert_eq!(
            second.expires_at.duration_since(second.created_at),
            BACKLOG_TTL
        );
    }

    #[test]
    fn empty_messages_are_ignored() {
        let mut state = NotificationState::new();

        assert!(!state.enqueue(NotificationLevel::Info, "   ".to_string(), Instant::now()));
        assert!(state.active().is_none());
    }

    #[test]
    fn backlog_messages_keep_short_ttl_until_queue_clears() {
        let mut state = NotificationState::new();
        let now = Instant::now();

        assert!(state.enqueue(NotificationLevel::Info, "one".to_string(), now));
        assert!(state.enqueue(NotificationLevel::Warn, "two".to_string(), now));

        let first_expiry = state.active().expect("active").expires_at;
        assert!(state.prune_and_advance(first_expiry));
        let second = state.active().expect("active");
        assert_eq!(
            second.expires_at.duration_since(second.created_at),
            BACKLOG_TTL
        );

        let second_expiry = second.expires_at;
        assert!(state.prune_and_advance(second_expiry));
        assert!(state.active().is_none());

        let after_clear = second_expiry + Duration::from_millis(10);
        assert!(state.enqueue(NotificationLevel::Info, "three".to_string(), after_clear));
        let third = state.active().expect("active");
        assert_eq!(
            third.expires_at.duration_since(third.created_at),
            NORMAL_TTL
        );
    }
}
