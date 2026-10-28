//! Keyboard lighting: one abstraction over every Lighting Backend.
//!
//! A [`LightingBackend`] declares what it can do: its Lighting Effects (each
//! saying which parameters it uses), its directions, how many zones it colours
//! and whether it can pick colours at random. A [`Lighting`] is one complete
//! configuration for a backend, and [`apply`] sends it to the hardware. The UI
//! only reads the declarations, so a new backend needs no UI changes.
//!
//! Saved Lighting, the persisted [`Lighting`], lives in [`saved`].

pub mod saved;
mod usb_hid;

use std::fmt;

use anyhow::Result;

use crate::keyboard::{KeyboardTransport, UsbAccess, keyboard_access};
use crate::machine::MachineRoot;

/// The mechanism that drives a keyboard's RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LightingBackend {
    /// The PH16-71-family Chicony keyboard (`04f2:0117`), spoken to directly over USB.
    UsbHid,
}

impl LightingBackend {
    /// The name the header shows, e.g. "USB HID".
    pub fn name(self) -> &'static str {
        match self {
            Self::UsbHid => "USB HID",
        }
    }

    /// The Lighting Effects this backend offers, in the order the UI lists them.
    pub fn effects(self) -> &'static [Effect] {
        match self {
            Self::UsbHid => &usb_hid::EFFECTS,
        }
    }

    /// The effect called `name`, if this backend has it.
    pub fn effect(self, name: &str) -> Option<&'static Effect> {
        self.effects().iter().find(|effect| effect.name == name)
    }

    /// The directions a directional effect can travel in.
    pub fn directions(self) -> &'static [&'static str] {
        match self {
            Self::UsbHid => &usb_hid::DIRECTIONS,
        }
    }

    /// How many zones the backend colours separately (1 for a whole keyboard).
    pub fn zones(self) -> usize {
        match self {
            Self::UsbHid => 1,
        }
    }

    /// Whether the keyboard itself can pick the colours ([`Colour::Random`]).
    pub fn has_random_colour(self) -> bool {
        match self {
            Self::UsbHid => true,
        }
    }

    /// The lighting a keyboard gets before anything is saved: static white at 30%.
    pub fn default_lighting(self) -> Lighting {
        Lighting {
            backend: self,
            effect: self.effect("Static").unwrap_or_else(|| &self.effects()[0]),
            colours: vec![Colour::Rgb(Rgb::WHITE); self.zones()],
            brightness: 30,
            speed: 50,
            direction: self.directions()[0],
        }
    }
}

/// A Lighting Effect: an animation or static pattern the keyboard renders itself.
#[derive(Debug, PartialEq, Eq)]
pub struct Effect {
    pub name: &'static str,
    /// The backend's own number for the effect (a USB HID opcode, a Four-Zone mode).
    pub code: u8,
    /// The parameters the effect uses; the UI hides the rest.
    pub uses: Uses,
}

/// Which parameters of a [`Lighting`] an [`Effect`] pays attention to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uses {
    pub colour: bool,
    pub brightness: bool,
    pub speed: bool,
    pub direction: bool,
}

/// A 24-bit RGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const WHITE: Self = Self::new(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses exactly six hex digits, e.g. `ff8800` (no `#`).
    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let value = u32::from_str_radix(hex, 16).ok()?;
        Some(Self::new(
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ))
    }
}

impl fmt::Display for Rgb {
    /// `#RRGGBB`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// The curated palette the Colour row offers, in swatch order.
pub const PALETTE: [(&str, Rgb); 10] = [
    ("Red", Rgb::new(255, 70, 70)),
    ("Orange", Rgb::new(255, 142, 45)),
    ("Gold", Rgb::new(250, 204, 21)),
    ("Emerald", Rgb::new(52, 211, 153)),
    ("Cyan", Rgb::new(34, 211, 238)),
    ("Blue", Rgb::new(96, 165, 250)),
    ("Violet", Rgb::new(167, 139, 250)),
    ("Magenta", Rgb::new(232, 121, 249)),
    ("Pink", Rgb::new(244, 114, 182)),
    ("White", Rgb::WHITE),
];

/// One zone's colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Colour {
    Rgb(Rgb),
    /// The keyboard picks; only on backends that can ([`LightingBackend::has_random_colour`]).
    Random,
}

impl Colour {
    /// The palette name, "Random", or `#RRGGBB` for a custom colour.
    pub fn name(self) -> String {
        match self {
            Self::Random => "Random".to_string(),
            Self::Rgb(rgb) => PALETTE
                .iter()
                .find(|(_, colour)| *colour == rgb)
                .map_or_else(|| rgb.to_string(), |(name, _)| name.to_string()),
        }
    }
}

/// One complete lighting configuration for a backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lighting {
    pub backend: LightingBackend,
    /// One of `backend.effects()`.
    pub effect: &'static Effect,
    /// One colour per zone, `backend.zones()` of them.
    pub colours: Vec<Colour>,
    /// Percent, 0..=100.
    pub brightness: u8,
    /// Percent, 0..=100; 100 is fastest.
    pub speed: u8,
    /// One of `backend.directions()`.
    pub direction: &'static str,
}

impl Lighting {
    /// The first zone's colour: the whole keyboard's on a single-zone backend.
    pub fn colour(&self) -> Colour {
        self.colours
            .first()
            .copied()
            .unwrap_or(Colour::Rgb(Rgb::WHITE))
    }
}

/// Finds the keyboard's Lighting Backend on `machine`, if it has one Arch-Sense drives.
pub fn detect(machine: &MachineRoot) -> Option<LightingBackend> {
    (keyboard_access(machine) != UsbAccess::NotFound).then_some(LightingBackend::UsbHid)
}

/// Sends `lighting` to its backend's hardware: USB HID packets go through
/// `keyboard`, Linuwu-Sense attributes resolve under `machine`.
pub fn apply(
    lighting: &Lighting,
    _machine: &MachineRoot,
    keyboard: &mut dyn KeyboardTransport,
) -> Result<()> {
    match lighting.backend {
        LightingBackend::UsbHid => keyboard.send(&usb_hid::packets(lighting)),
    }
}
