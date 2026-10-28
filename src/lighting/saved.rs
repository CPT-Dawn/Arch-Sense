//! Saved Lighting: the last [`Lighting`] the user applied, as a versioned JSON
//! document in Arch-Sense's state folder (`/var/lib/arch-sense/lighting.json`).
//!
//! ```json
//! { "version": 2, "backend": "usb-hid", "effect": "Wave", "colours": ["#1234AB"],
//!   "brightness": 85, "speed": 20, "direction": "Clockwise" }
//! ```
//!
//! Effects, directions and backends are stored by name, so reordering a list
//! never changes what a saved file means. Colours are `#RRGGBB` or `random`,
//! one per zone. The folder is setgid `arch-sense` and the file is written
//! group-writable, so every member of the group can save over it.
//!
//! v1 kept palette and effect *indices* in `config.json`. When there is no
//! Saved Lighting yet, [`load`] migrates that file and writes the v2 document.

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::{Colour, Lighting, LightingBackend, PALETTE, Rgb};
use crate::constants::setup_hint;
use crate::machine::MachineRoot;

const FILE: &str = "lighting.json";
const V1_FILE: &str = "config.json";
const VERSION: u32 = 2;
/// rw-rw-r--: the whole `arch-sense` group may save over it.
const MODE: u32 = 0o664;

/// Where the Saved Lighting lives on `machine`.
pub fn path(machine: &MachineRoot) -> PathBuf {
    machine.state_dir().join(FILE)
}

/// The Saved Lighting, `None` if nothing was ever saved. A v1 config found in
/// its place is migrated and, where permissions allow, saved in the v2 format.
pub fn load(machine: &MachineRoot) -> Result<Option<Lighting>> {
    let path = path(machine);
    match fs::read_to_string(&path) {
        Ok(text) => parse(&text)
            .map(Some)
            .with_context(|| format!("reading Saved Lighting from {}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => migrate_v1(machine),
        Err(error) => Err(anyhow!(error)).with_context(|| format!("reading {}", path.display())),
    }
}

/// Saves `lighting` as the Saved Lighting, replacing the file atomically.
pub fn save(machine: &MachineRoot, lighting: &Lighting) -> Result<()> {
    let dir = machine.state_dir();
    let json = serde_json::to_string_pretty(&Document::from(lighting))?;
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(&dir)?;
        let mut file = tempfile::Builder::new()
            .prefix(".lighting-")
            .tempfile_in(&dir)?;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(MODE))?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;
        file.persist(path(machine))?;
        Ok(())
    };
    write().map_err(|error| {
        let hint = if error.kind() == ErrorKind::PermissionDenied {
            format!("; {}", setup_hint())
        } else {
            String::new()
        };
        anyhow!("saving to {} failed: {error}{hint}", dir.display())
    })
}

/// The on-disk shape of Saved Lighting v2.
#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    backend: String,
    effect: String,
    colours: Vec<String>,
    brightness: u8,
    speed: u8,
    direction: String,
}

impl From<&Lighting> for Document {
    fn from(lighting: &Lighting) -> Self {
        Self {
            version: VERSION,
            backend: backend_id(lighting.backend).to_string(),
            effect: lighting.effect.name.to_string(),
            colours: lighting.colours.iter().map(|&c| colour_text(c)).collect(),
            brightness: lighting.brightness,
            speed: lighting.speed,
            direction: lighting.direction.to_string(),
        }
    }
}

fn parse(text: &str) -> Result<Lighting> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    match value.get("version").and_then(serde_json::Value::as_u64) {
        Some(version) if version == VERSION as u64 => {}
        Some(version) => bail!("version {version} is not one this Arch-Sense understands"),
        None => bail!("no version"),
    }
    let document: Document = serde_json::from_value(value)?;

    let backend = backend_from_id(&document.backend)
        .ok_or_else(|| anyhow!("unknown Lighting Backend {:?}", document.backend))?;
    let effect = backend
        .effect(&document.effect)
        .ok_or_else(|| anyhow!("unknown Lighting Effect {:?}", document.effect))?;
    let direction = backend
        .directions()
        .iter()
        .find(|&&direction| direction == document.direction)
        .ok_or_else(|| anyhow!("unknown direction {:?}", document.direction))?;
    let colours = document
        .colours
        .iter()
        .map(|text| parse_colour(text).ok_or_else(|| anyhow!("bad colour {text:?}")))
        .collect::<Result<Vec<_>>>()?;
    if colours.len() != backend.zones() {
        bail!("{} colours for {} zones", colours.len(), backend.zones());
    }

    Ok(Lighting {
        backend,
        effect,
        colours,
        brightness: document.brightness.min(100),
        speed: document.speed.min(100),
        direction,
    })
}

fn backend_id(backend: LightingBackend) -> &'static str {
    match backend {
        LightingBackend::UsbHid => "usb-hid",
    }
}

fn backend_from_id(id: &str) -> Option<LightingBackend> {
    match id {
        "usb-hid" => Some(LightingBackend::UsbHid),
        _ => None,
    }
}

fn colour_text(colour: Colour) -> String {
    match colour {
        Colour::Random => "random".to_string(),
        Colour::Rgb(rgb) => rgb.to_string(),
    }
}

fn parse_colour(text: &str) -> Option<Colour> {
    match text {
        "random" => Some(Colour::Random),
        _ => Rgb::from_hex(text.strip_prefix('#')?).map(Colour::Rgb),
    }
}

/// v1's `config.json`: indices into its effect list, palette and directions.
#[derive(Deserialize)]
struct V1Config {
    rgb: V1Rgb,
}

#[derive(Deserialize)]
struct V1Rgb {
    effect: usize,
    color: usize,
    brightness: u8,
    speed: u8,
    direction: usize,
}

/// v1's palette index for "keyboard picks at random"; 0..=9 match [`PALETTE`].
const V1_RANDOM: usize = 10;

fn migrate_v1(machine: &MachineRoot) -> Result<Option<Lighting>> {
    let v1_path = machine.state_dir().join(V1_FILE);
    let text = match fs::read_to_string(&v1_path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(anyhow!(error)).with_context(|| format!("reading {}", v1_path.display()));
        }
    };
    let v1: V1Config = serde_json::from_str(&text)
        .with_context(|| format!("migrating the v1 config at {}", v1_path.display()))?;
    let v1 = v1.rgb;

    // v1 only ever drove the USB HID keyboard, and its effect list is v2's.
    let backend = LightingBackend::UsbHid;
    let effects = backend.effects();
    let directions = backend.directions();
    let colour = match PALETTE.get(v1.color) {
        Some((_, rgb)) => Colour::Rgb(*rgb),
        None if v1.color >= V1_RANDOM => Colour::Random,
        None => Colour::Rgb(Rgb::WHITE),
    };
    let lighting = Lighting {
        backend,
        effect: &effects[v1.effect.min(effects.len() - 1)],
        colours: vec![colour; backend.zones()],
        brightness: v1.brightness.min(100),
        speed: v1.speed.min(100),
        direction: directions[v1.direction.min(directions.len() - 1)],
    };

    // Best effort: a user who can't write the state folder still gets the migrated lighting.
    let _ = save(machine, &lighting);
    Ok(Some(lighting))
}
