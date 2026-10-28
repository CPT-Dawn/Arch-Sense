use std::io::{self, Write};

use anyhow::Result;

use crate::doctor::{self, Membership};
use crate::keyboard::KeyboardTransport;
use crate::lighting::{self, saved};
use crate::machine::MachineRoot;

/// Boot Restore: reapplies the Saved Lighting, quietly doing nothing when
/// nothing is saved or the keyboard it was saved for isn't here.
pub fn apply_saved_config(
    machine: &MachineRoot,
    keyboard: &mut dyn KeyboardTransport,
) -> Result<()> {
    let Some(lighting) = saved::load(machine)? else {
        eprintln!("arch-sense: no Saved Lighting, nothing to restore");
        return Ok(());
    };

    match lighting::detect(machine).backend() {
        None => {
            eprintln!("arch-sense: no supported keyboard found, nothing to restore");
            return Ok(());
        }
        Some(backend) if backend != lighting.backend => {
            eprintln!(
                "arch-sense: Saved Lighting is for a {} keyboard, but this one is {}; skipping",
                lighting.backend.name(),
                backend.name()
            );
            return Ok(());
        }
        Some(_) => {}
    }

    match lighting::apply(&lighting, machine, keyboard) {
        Ok(()) => {
            eprintln!("arch-sense: keyboard lighting restored");
            Ok(())
        }
        Err(error) => {
            eprintln!("arch-sense: restoring keyboard lighting failed: {error}");
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
