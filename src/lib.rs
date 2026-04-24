pub mod app;
pub mod config;
pub mod constants;
pub mod permissions;
pub mod rgb_settings;
pub mod system;
pub mod theme;
pub mod ui;

use anyhow::Result;

use app::App;
use config::{config_path, AppConfig};
use permissions::UsbAccess;
use rgb_settings::{is_kb_present, send_rgb, RgbState};

pub fn run() -> Result<()> {
    let terminal = ratatui::init();
    let app = App::new();

    if matches!(app.rgb.kb_access, UsbAccess::Accessible) {
        let _ = send_rgb(&app.rgb);
    }

    let result = app.run(terminal);
    ratatui::restore();
    result
}

pub fn print_help() {
    eprintln!("Arch-Sense - Acer Predator Control Center\n");
    eprintln!("Usage:");
    eprintln!("  arch-sense                         Launch TUI");
    eprintln!("  arch-sense --install-permissions   One-time setup for running without sudo");
    eprintln!("  arch-sense --doctor                Check hardware permissions");
    eprintln!("  arch-sense --apply                 Apply saved RGB settings (for boot/systemd)");
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
    let rgb = RgbState::from_config(&config.rgb);

    if !is_kb_present() {
        eprintln!("arch-sense: Keyboard not found (VID:04F2 PID:0117)");
        std::process::exit(0);
    }

    match send_rgb(&rgb) {
        Ok(msg) => {
            eprintln!("arch-sense: {msg}");
            Ok(())
        }
        Err(e) => {
            eprintln!("arch-sense: RGB apply failed: {e}");
            Err(e)
        }
    }
}
