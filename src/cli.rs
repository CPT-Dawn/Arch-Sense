use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "arch-sense",
    author,
    version,
    about = "Terminal control center for Acer Predator and Nitro laptops, via Linuwu-Sense",
    long_about = "Terminal control center for Acer Predator and Nitro laptops, built on the Linuwu-Sense kernel module: Thermal Profile, Fans (Auto or Max), battery care, keyboard lighting and the other controls your laptop exposes."
)]
pub struct Cli {
    /// Boot Restore: reapply the Saved Lighting without launching the TUI
    #[arg(long)]
    pub apply: bool,

    /// Try the full UI on a simulated laptop; touches nothing on this machine
    #[arg(long, conflicts_with = "apply")]
    pub demo: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Explain what isn't working and how to fix it; exits non-zero on errors
    Doctor,
}
