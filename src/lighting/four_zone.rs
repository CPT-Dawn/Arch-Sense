//! The Four-Zone Lighting Backend: Linuwu-Sense's `four_zoned_kb` attributes.
//!
//! Two files take the writes, both a single comma-separated line:
//!
//! ```text
//! four_zone_mode   mode,speed,brightness,direction,red,green,blue     decimal
//! per_zone_mode    rrggbb,rrggbb,rrggbb,rrggbb,brightness             hex, then decimal
//! ```
//!
//! `mode` 0..=7 is the effect, `speed` 0..=9, `brightness` 0..=100, `direction`
//! 1 (right to left) or 2 (left to right). Parameters an effect ignores are sent
//! as 0, except the direction of Wave and Shifting, which the driver rejects
//! unless it is 1 or 2. Per-zone colours exist only for a static colour: they go
//! through `per_zone_mode`, and writing `four_zone_mode` afterwards switches the
//! keyboard back to an effect.

use std::fs;
use std::io::ErrorKind;

use anyhow::{Result, anyhow};

use super::{Colour, Effect, Lighting, Rgb, Uses};
use crate::constants::setup_hint;
use crate::machine::MachineRoot;

pub(super) const ZONES: usize = 4;
/// "Right" is left-to-right travel (2), "Left" is right-to-left (1).
pub(super) const DIRECTIONS: [&str; 2] = ["Right", "Left"];
const SPEED_MAX: u16 = 9;

const fn effect(
    name: &'static str,
    code: u8,
    colour: bool,
    speed: bool,
    direction: bool,
    zones: bool,
) -> Effect {
    Effect {
        name,
        code,
        uses: Uses {
            colour,
            brightness: true,
            speed,
            direction,
            zones,
        },
    }
}

pub(super) const EFFECTS: [Effect; 8] = [
    effect("Static", 0, true, false, false, true),
    effect("Breathing", 1, true, false, false, false),
    effect("Neon", 2, false, true, false, false),
    effect("Wave", 3, false, true, true, false),
    effect("Shifting", 4, true, true, true, false),
    effect("Zoom", 5, true, true, false, false),
    effect("Meteor", 6, true, true, false, false),
    effect("Twinkling", 7, true, true, false, false),
];

/// Puts `lighting` on the keyboard through the attribute files under `machine`.
pub(super) fn write(lighting: &Lighting, machine: &MachineRoot) -> Result<()> {
    let (file, line) = match per_zone_line(lighting) {
        Some(line) => ("per_zone_mode", line),
        None => ("four_zone_mode", mode_line(lighting)),
    };
    let path = machine.four_zone_kb_dir().join(file);
    fs::write(&path, line).map_err(|error| {
        let hint = if error.kind() == ErrorKind::PermissionDenied {
            format!("; {}", setup_hint())
        } else {
            String::new()
        };
        anyhow!("writing {} failed: {error}{hint}", path.display())
    })
}

/// The `per_zone_mode` line, when the zones differ and the effect can show it.
fn per_zone_line(lighting: &Lighting) -> Option<String> {
    let first = lighting.colours.first()?;
    if !lighting.effect.uses.zones || lighting.colours.iter().all(|c| c == first) {
        return None;
    }
    let zones: Vec<String> = lighting
        .colours
        .iter()
        .map(|&colour| {
            let rgb = rgb_of(colour);
            format!("{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
        })
        .collect();
    Some(format!(
        "{},{}\n",
        zones.join(","),
        lighting.brightness.min(100)
    ))
}

/// The `four_zone_mode` line for the whole keyboard.
fn mode_line(lighting: &Lighting) -> String {
    let effect = lighting.effect;
    let speed = if effect.uses.speed {
        (lighting.speed.min(100) as u16 * SPEED_MAX + 50) / 100
    } else {
        0
    };
    let direction = match (effect.uses.direction, lighting.direction) {
        (false, _) => 0,
        (true, "Right") => 2,
        (true, _) => 1,
    };
    let rgb = if effect.uses.colour {
        rgb_of(lighting.colour())
    } else {
        Rgb::new(0, 0, 0)
    };
    format!(
        "{},{speed},{},{direction},{},{},{}\n",
        effect.code,
        lighting.brightness.min(100),
        rgb.r,
        rgb.g,
        rgb.b
    )
}

/// Four-Zone has no random colour; should one sneak in, it is white.
fn rgb_of(colour: Colour) -> Rgb {
    match colour {
        Colour::Rgb(rgb) => rgb,
        Colour::Random => Rgb::WHITE,
    }
}
