use anyhow::Result;
use crate::config::AppConfig;
use crate::models::RgbSettings;
use crate::permissions;
use crate::hardware;

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
