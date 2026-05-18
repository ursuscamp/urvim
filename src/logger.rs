use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init<P: AsRef<Path>>(log_file: P) -> WorkerGuard {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file.as_ref())
        .expect("Failed to open log file");
    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug,ignore=off"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .without_time(),
        )
        .init();

    guard
}
