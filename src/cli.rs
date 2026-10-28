use clap::Parser;

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
}
