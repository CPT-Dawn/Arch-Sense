use std::io::{self, Write};

use anyhow::Result;

use crate::config::AppConfig;
use crate::doctor::{self, Membership};
use crate::hardware;
use crate::keyboard::{KeyboardTransport, UsbAccess, keyboard_access};
use crate::machine::MachineRoot;
use crate::models::RgbSettings;

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

/// `arch-sense doctor`: writes the Doctor's checklist to `out` and returns the
/// exit code, non-zero exactly when a check failed with an error.
pub fn doctor(
    machine: &MachineRoot,
    membership: Membership,
    out: &mut impl Write,
) -> io::Result<i32> {
    let report = doctor::diagnose(machine, membership);
    writeln!(out, "Arch-Sense Doctor (v{})", env!("CARGO_PKG_VERSION"))?;
    writeln!(out)?;
    write!(out, "{report}")?;
    Ok(report.exit_code())
}
