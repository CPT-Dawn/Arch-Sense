pub mod app;
pub mod config;
pub mod constants;
pub mod hardware;
pub mod models;
pub mod permissions;
pub mod theme;
pub mod ui;

use anyhow::Result;

use app::App;
use config::{config_path, AppConfig};
use models::RgbSettings;

pub fn run() -> Result<()> {
    let app = App::new()?;
    let terminal = ratatui::init();
    let result = app.run(terminal);
    ratatui::restore();
    result
}

pub fn print_help() {
    eprintln!("Arch-Sense - Acer Predator hardware control center\n");
    eprintln!("Usage:");
    eprintln!("  arch-sense                         Launch the modern single-screen TUI");
    eprintln!("  arch-sense --install-permissions   One-time setup for running without sudo");
    eprintln!("  arch-sense --doctor                Check hardware permissions");
    eprintln!("  arch-sense --apply                 Apply saved RGB settings without the TUI");
    eprintln!("\nConfig: {}", config_path().display());
    eprintln!("Systemd: sudo cp arch-sense.service /etc/systemd/system/");
    eprintln!("         sudo systemctl enable --now arch-sense");
}

pub fn print_permission_report() -> Result<()> {
    permissions::print_permission_report()
}

pub fn install_permissions() -> Result<()> {
    permissions::install_permissions()
}

pub fn install_permissions_as_root() -> Result<()> {
    permissions::install_permissions_as_root()
}

pub fn apply_permissions() -> Result<()> {
    permissions::apply_permissions_as_root()
}

pub fn apply_saved_config() -> Result<()> {
    let config = AppConfig::load();
    let rgb = RgbSettings::from_config(&config.rgb);

    if !hardware::is_keyboard_present() {
        eprintln!("arch-sense: keyboard not found (VID:04F2 PID:0117)");
        return Ok(());
    }

    match hardware::apply_rgb_settings(&rgb) {
        Ok(message) => {
            eprintln!("arch-sense: {message}");
            Ok(())
        }
        Err(error) => {
            eprintln!("arch-sense: RGB apply failed: {error}");
            Err(error)
        }
    }
}
