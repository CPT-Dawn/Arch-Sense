use anyhow::Result;

use crate::config::AppConfig;
use crate::hardware;
use crate::keyboard::{KeyboardTransport, UsbAccess, keyboard_access};
use crate::machine::MachineRoot;
use crate::models::RgbSettings;
use crate::permissions;

pub fn print_permission_report(machine: &MachineRoot) -> Result<()> {
    permissions::print_permission_report(machine)
}

pub fn install_permissions(machine: &MachineRoot) -> Result<()> {
    permissions::install_permissions(machine)
}

pub fn install_permissions_as_root(machine: &MachineRoot) -> Result<()> {
    permissions::install_permissions_as_root(machine)
}

pub fn apply_permissions(machine: &MachineRoot) -> Result<()> {
    permissions::apply_permissions_as_root(machine)
}

pub fn apply_saved_config(
    machine: &MachineRoot,
    keyboard: &mut dyn KeyboardTransport,
) -> Result<()> {
    let config = AppConfig::load(machine);
    let rgb = RgbSettings::from_config(&config.rgb);

    if keyboard_access(machine) == UsbAccess::NotFound {
        eprintln!("arch-sense: keyboard not found (VID:04F2 PID:0117)");
        return Ok(());
    }

    match hardware::apply_rgb_settings(&rgb, keyboard) {
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
