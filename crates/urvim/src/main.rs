use clap::Parser;
use std::io;

mod actions;
mod app;
mod plugin;
mod render;
mod startup;

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
