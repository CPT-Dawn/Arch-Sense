//! Capability detection: what this laptop's Sense Interface actually offers.
//!
//! Linuwu-Sense exposes one attribute folder per Sense Interface. Which attributes
//! appear depends on the laptop line, and some firmware exposes an attribute that
//! then fails every read (legacy Predators such as the PH315-52). So a
//! [`Capability`] counts as present only if its attribute exists *and* reads.

use std::fs;
use std::path::PathBuf;

use crate::machine::MachineRoot;

/// The family of controls Linuwu-Sense exposes for a laptop line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SenseInterface {
    Predator,
    Nitro,
    /// Linuwu-Sense isn't loaded, or doesn't recognise this laptop.
    None,
}

impl SenseInterface {
    /// The folder holding this interface's attributes, if it has one.
    fn dir(self, machine: &MachineRoot) -> Option<PathBuf> {
        match self {
            Self::Predator => Some(machine.predator_sense_dir()),
            Self::Nitro => Some(machine.nitro_sense_dir()),
            Self::None => None,
        }
    }

    /// One attribute file of this interface, e.g. `fan_speed`.
    pub fn attribute(self, machine: &MachineRoot, name: &str) -> Option<PathBuf> {
        self.dir(machine).map(|dir| dir.join(name))
    }
}

/// One hardware control a laptop can expose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    ThermalProfile,
    Fans,
    ChargeLimit,
    UsbCharging,
    BatteryCalibration,
    BootAnimation,
    LcdOverdrive,
    BacklightTimeout,
}

impl Capability {
    /// Every Capability, in the order the UI lists them. The settings list
    /// takes its order from here; nothing else spells it out.
    pub const ALL: [Self; 8] = [
        Self::ThermalProfile,
        Self::Fans,
        Self::ChargeLimit,
        Self::UsbCharging,
        Self::BatteryCalibration,
        Self::BootAnimation,
        Self::LcdOverdrive,
        Self::BacklightTimeout,
    ];

    /// The file this Capability is read from and written to on `machine`, given
    /// the laptop's Sense Interface. `None` when the interface has no such file.
    pub fn path(self, machine: &MachineRoot, interface: SenseInterface) -> Option<PathBuf> {
        match self.sense_attribute() {
            Some(name) => interface.attribute(machine, name),
            None => Some(machine.platform_profile()),
        }
    }

    /// The attribute file inside the Sense Interface folder, for Capabilities that live there.
    fn sense_attribute(self) -> Option<&'static str> {
        match self {
            Self::ThermalProfile => None,
            Self::Fans => Some("fan_speed"),
            Self::ChargeLimit => Some("battery_limiter"),
            Self::BatteryCalibration => Some("battery_calibration"),
            Self::UsbCharging => Some("usb_charging"),
            Self::BootAnimation => Some("boot_animation_sound"),
            Self::LcdOverdrive => Some("lcd_override"),
            Self::BacklightTimeout => Some("backlight_timeout"),
        }
    }
}

/// A Thermal Profile the firmware offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThermalProfile(String);

impl ThermalProfile {
    /// The firmware's name for it, verbatim (e.g. `balanced-performance`): what
    /// `platform_profile` reads and takes.
    pub fn name(&self) -> &str {
        &self.0
    }

    /// The name as people say it, e.g. "Balanced Performance".
    pub fn label(&self) -> String {
        self.0
            .split('-')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(chars).collect()
                })
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl PartialEq<&str> for ThermalProfile {
    fn eq(&self, name: &&str) -> bool {
        self.0 == *name
    }
}

/// The read-only Turbo indicator: whether the Turbo key has engaged turbo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turbo {
    Off,
    Engaged,
}

/// Everything Arch-Sense detected about the laptop's controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Laptop {
    pub sense_interface: SenseInterface,
    /// The Thermal Profiles the firmware offers, in its order. Empty when
    /// there is no Thermal Profile.
    pub thermal_profiles: Vec<ThermalProfile>,
    /// The Turbo indicator, when the laptop has a Turbo key that reads.
    pub turbo: Option<Turbo>,
    capabilities: Vec<Capability>,
}

impl Laptop {
    /// The Capabilities present, in UI order.
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }

    pub fn has(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Inspects `machine` and describes the laptop's Sense Interface and Capabilities.
pub fn detect(machine: &MachineRoot) -> Laptop {
    let sense_interface = [SenseInterface::Predator, SenseInterface::Nitro]
        .into_iter()
        .find(|interface| interface.dir(machine).is_some_and(|dir| dir.is_dir()))
        .unwrap_or(SenseInterface::None);

    let thermal_profiles: Vec<ThermalProfile> = read(machine.platform_profile())
        .and_then(|_| read(machine.platform_profile_choices()))
        .map(|choices| {
            choices
                .split_whitespace()
                .map(|name| ThermalProfile(name.to_owned()))
                .collect()
        })
        .unwrap_or_default();

    let capabilities = Capability::ALL
        .into_iter()
        .filter(|&capability| match capability.sense_attribute() {
            Some(name) => sense_interface
                .attribute(machine, name)
                .and_then(read)
                .is_some(),
            None => !thermal_profiles.is_empty(),
        })
        .collect();

    let turbo = read(machine.turbo_state()).and_then(|state| match state.trim() {
        "0" => Some(Turbo::Off),
        "1" => Some(Turbo::Engaged),
        _ => None,
    });

    Laptop {
        sense_interface,
        thermal_profiles,
        turbo,
        capabilities,
    }
}

/// The contents of `path`, or `None` if it is missing or the read fails.
fn read(path: PathBuf) -> Option<String> {
    fs::read_to_string(path).ok()
}
