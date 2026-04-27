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
    /// Check hardware permissions and system status
    #[arg(long)]
    pub doctor: bool,

    /// One-time setup for running without sudo
    #[arg(long)]
    pub install_permissions: bool,

    /// Apply saved RGB settings without launching the TUI
    #[arg(long)]
    pub apply: bool,

    /// Internal: Run permission installation as root (triggered via pkexec)
    #[arg(long, hide = true)]
    pub install_permissions_root: bool,

    /// Internal: Directly apply permissions to sysfs and config directories
    #[arg(long, hide = true)]
    pub apply_permissions: bool,
}
