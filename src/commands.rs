use anyhow::Result;

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

    match lighting::detect(machine) {
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
