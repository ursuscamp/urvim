//! Notification data model types.

use std::time::Instant;

/// Notification severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Informational notification.
    Info,
    /// Warning notification.
    Warn,
    /// Error notification.
    Error,
}

/// A notification currently shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationMessage {
    /// Notification severity.
    pub level: NotificationLevel,
    /// Rendered notification text.
    pub text: String,
    /// Message creation timestamp.
    pub created_at: Instant,
    /// Timestamp at which the message expires.
    pub expires_at: Instant,
}

impl NotificationMessage {
    /// Creates a notification message with an explicit TTL.
    pub fn new(
        level: NotificationLevel,
        text: String,
        created_at: Instant,
        ttl: std::time::Duration,
    ) -> Self {
        Self {
            level,
            text,
            created_at,
            expires_at: created_at + ttl,
        }
    }

    /// Returns true when this message is expired at `now`.
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.expires_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingNotification {
    pub(super) level: NotificationLevel,
    pub(super) text: String,
    pub(super) created_at: Instant,
    pub(super) backlog_ttl: bool,
}
