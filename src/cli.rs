use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "arch-sense",
    author,
    version,
    about = "Acer Predator hardware control center",
    long_about = "A modern TUI and CLI tool for managing Acer Predator hardware on Arch Linux, including keyboard RGB, thermal profiles, fan speeds, and battery health settings."
)]
pub struct Cli {
    /// Apply saved RGB settings without launching the TUI
    #[arg(long)]
    pub apply: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Explain what isn't working and how to fix it; exits non-zero on errors
    Doctor,
}
