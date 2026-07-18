use clap::Parser;
use std::io;

mod actions;
mod app;
mod plugin;
mod render;
mod startup;

#[cfg(test)]
fn theme_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
// The core dependency uses a process-global buffer pool in application tests.
fn buffer_pool_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[derive(Parser)]
#[command(name = "urvim")]
#[command(version = "0.1.0")]
#[command(about = "A terminal-based text editor", long_about = None)]
struct Cli {
    #[arg(long)]
    theme: Option<String>,
    #[arg(long = "no-syntax")]
    no_syntax: bool,
    files: Vec<urvim_core::cli::CliFileSpec>,
}

fn main() -> io::Result<()> {
    app::run(Cli::parse())
}
