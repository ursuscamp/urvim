pub mod action;
pub mod background;
pub mod buffer;
pub mod cli;
pub mod command;
pub mod config;
pub mod diagnostics;
pub mod editor;
pub mod globals;
pub mod icon;
pub mod layout;
pub mod logger;
pub mod lsp;
pub mod motion;
pub mod notification;
pub mod path;
pub mod register;
pub mod screen;
pub mod session;
pub mod status_bar;
pub mod ui;
pub mod widget;
pub mod window;
pub mod window_group;

mod jumplist;

pub use layout::Layout;
pub use path::AbsolutePath;
pub use window_group::WindowGroup;

/// Logs and enqueues a notification with an explicit level.
#[macro_export]
macro_rules! notify {
    ($level:expr, $($arg:tt)+) => {{
        $crate::notification::notify_message($level, format!($($arg)+));
    }};
}

/// Logs and enqueues an info-level notification.
#[macro_export]
macro_rules! notify_info {
    ($($arg:tt)+) => {{
        $crate::notification::notify_message(
            $crate::notification::NotificationLevel::Info,
            format!($($arg)+),
        );
    }};
}

/// Logs and enqueues a warning-level notification.
#[macro_export]
macro_rules! notify_warn {
    ($($arg:tt)+) => {{
        $crate::notification::notify_message(
            $crate::notification::NotificationLevel::Warn,
            format!($($arg)+),
        );
    }};
}

/// Logs and enqueues an error-level notification.
#[macro_export]
macro_rules! notify_error {
    ($($arg:tt)+) => {{
        $crate::notification::notify_message(
            $crate::notification::NotificationLevel::Error,
            format!($($arg)+),
        );
    }};
}

#[cfg(test)]
mod tests {
    use crate::globals;
    use crate::notification::NotificationLevel;
    use std::time::Instant;

    #[test]
    fn notify_macros_route_messages_with_expected_levels() {
        let _guard = globals::notification_test_lock();
        globals::clear_notifications();

        notify_info!("saved {}", 1);
        let info = globals::active_notification(Instant::now()).expect("info notification");
        assert_eq!(info.level, NotificationLevel::Info);
        assert!(info.text.starts_with("saved 1"));

        globals::clear_notifications();
        notify_warn!("warn {}", 2);
        let warn = globals::active_notification(Instant::now()).expect("warn notification");
        assert_eq!(warn.level, NotificationLevel::Warn);
        assert!(warn.text.starts_with("warn 2"));

        globals::clear_notifications();
        notify_error!("err {}", 3);
        let error = globals::active_notification(Instant::now()).expect("error notification");
        assert_eq!(error.level, NotificationLevel::Error);
        assert!(error.text.starts_with("err 3"));
    }

    #[test]
    fn notify_macro_respects_explicit_level() {
        let _guard = globals::notification_test_lock();
        globals::clear_notifications();

        notify!(NotificationLevel::Warn, "formatted {}", 42);
        let message = globals::active_notification(Instant::now()).expect("warn notification");
        assert_eq!(message.level, NotificationLevel::Warn);
        assert_eq!(message.text, "formatted 42");
    }
}
