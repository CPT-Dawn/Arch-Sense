//! Fake hardware trees: a [`MachineRoot`] in a temp directory, laid out like a real laptop.
//!
//! Integration tests use these to drive Arch-Sense without the hardware, and demo
//! mode uses them to run the full UI on a simulated laptop. Each builder method
//! adds one piece of hardware; pieces compose, and later ones win on conflicts.
//!
//! ```no_run
//! use arch_sense::fixture::FakeMachine;
//!
//! let fake = FakeMachine::builder()
//!     .predator_sense()
//!     .sensors()
//!     .usb_hid_keyboard()
//!     .build()?;
//! let machine = fake.machine(); // hand this to the app
//! # Ok::<(), std::io::Error>(())
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::keyboard::{USB_HID_PRODUCT_ID, USB_HID_VENDOR_ID};
use crate::machine::MachineRoot;

/// Every attribute a Predator (PH16-71) or Nitro v4 Sense Interface exposes.
const FULL_ATTRS: [(&str, &str); 7] = [
    ("backlight_timeout", "0"),
    ("battery_calibration", "0"),
    ("battery_limiter", "1"),
    ("boot_animation_sound", "0"),
    ("fan_speed", "0,0"),
    ("lcd_override", "0"),
    ("usb_charging", "0"),
];

type Step = Box<dyn FnOnce(&MachineRoot) -> io::Result<()>>;

/// A fake machine tree. The directory is deleted when this is dropped.
pub struct FakeMachine {
    dir: TempDir,
}

impl FakeMachine {
    pub fn builder() -> FakeMachineBuilder {
        FakeMachineBuilder { steps: Vec::new() }
    }

    /// The machine root to hand to Arch-Sense.
    pub fn machine(&self) -> MachineRoot {
        MachineRoot::new(self.dir.path())
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// Describes a fake laptop piece by piece; [`build`](Self::build) writes it to disk.
#[must_use]
pub struct FakeMachineBuilder {
    steps: Vec<Step>,
}

impl FakeMachineBuilder {
    /// Linuwu-Sense loaded with the Predator Sense Interface (every attribute
    /// a PH16-71 exposes) and an ACPI platform profile.
    pub fn predator_sense(self) -> Self {
        self.sense_interface(MachineRoot::predator_sense_dir, &FULL_ATTRS)
            .thermal_profile("balanced", "quiet balanced performance")
    }

    /// Linuwu-Sense loaded with the legacy Nitro Sense Interface (e.g. ANV15-51):
    /// no LCD overdrive and no boot animation.
    pub fn nitro_sense(self) -> Self {
        const ATTRS: [(&str, &str); 5] = [
            ("backlight_timeout", "0"),
            ("battery_calibration", "0"),
            ("battery_limiter", "0"),
            ("fan_speed", "0,0"),
            ("usb_charging", "0"),
        ];
        self.sense_interface(MachineRoot::nitro_sense_dir, &ATTRS)
            .thermal_profile("balanced", "low-power balanced performance")
    }

    /// Linuwu-Sense loaded with the Nitro v4 Sense Interface (e.g. AN16-41): the
    /// Nitro Sense folder carrying the full Predator attribute set.
    pub fn nitro_v4(self) -> Self {
        self.sense_interface(MachineRoot::nitro_sense_dir, &FULL_ATTRS)
            .thermal_profile(
                "balanced",
                "low-power quiet balanced balanced-performance performance",
            )
    }

    /// A laptop whose sensors work but Linuwu-Sense isn't loaded: no Sense
    /// Interface and no platform profile.
    pub fn module_missing(self) -> Self {
        self.sensors()
    }

    /// The firmware's Thermal Profile: the current one and the space-separated choices.
    pub fn thermal_profile(self, current: &str, choices: &str) -> Self {
        self.file(MachineRoot::platform_profile, current)
            .file(MachineRoot::platform_profile_choices, choices)
    }

    /// The laptop's DMI model name, e.g. `Predator PH16-71`.
    pub fn product_name(self, name: &str) -> Self {
        self.file(MachineRoot::dmi_product_name, format!("{name}\n"))
    }

    /// Makes `path` readable but not writable, the way an attribute looks to a
    /// user outside the `arch-sense` group. Root ignores this, so tests relying
    /// on it should skip when run as root.
    pub fn read_only(mut self, path: impl FnOnce(&MachineRoot) -> PathBuf + 'static) -> Self {
        self.steps.push(Box::new(move |machine| {
            let path = path(machine);
            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(path, permissions)
        }));
        self
    }

    /// The Turbo indicator Linuwu-Sense exposes on laptops with a Turbo key.
    pub fn turbo(self, engaged: bool) -> Self {
        self.file(MachineRoot::turbo_state, if engaged { "1" } else { "0" })
    }

    /// A legacy Predator (e.g. PH315-52): the full Predator Sense Interface, but
    /// the firmware rejects every read of the battery attributes.
    pub fn predator_legacy(self) -> Self {
        self.predator_sense()
            .unreadable(|m| m.predator_sense_attr("battery_limiter"))
            .unreadable(|m| m.predator_sense_attr("battery_calibration"))
    }

    /// hwmon chips for CPU and GPU temperature and two fans.
    pub fn sensors(self) -> Self {
        self.hwmon(
            0,
            "coretemp",
            &[("temp1_input", "52000"), ("temp1_label", "Package id 0")],
        )
        .hwmon(
            1,
            "amdgpu",
            &[("temp1_input", "47000"), ("temp1_label", "edge")],
        )
        .hwmon(
            2,
            "acer",
            &[
                ("fan1_input", "2400"),
                ("fan1_label", "cpu_fan"),
                ("fan2_input", "2600"),
                ("fan2_label", "gpu_fan"),
            ],
        )
    }

    /// The PH16-71-family USB HID keyboard (`04f2:0117`), openable by the current user.
    pub fn usb_hid_keyboard(self) -> Self {
        const BUS: u16 = 1;
        const DEVICE: u16 = 3;
        let device = |m: &MachineRoot| m.usb_devices_dir().join(format!("{BUS}-{DEVICE}"));

        self.file(
            move |m| device(m).join("idVendor"),
            format!("{USB_HID_VENDOR_ID:04x}"),
        )
        .file(
            move |m| device(m).join("idProduct"),
            format!("{USB_HID_PRODUCT_ID:04x}"),
        )
        .file(move |m| device(m).join("busnum"), BUS.to_string())
        .file(move |m| device(m).join("devnum"), DEVICE.to_string())
        .file(move |m| m.usb_device_node(BUS, DEVICE), "")
    }

    /// Writes the tree into a fresh temp directory.
    pub fn build(self) -> io::Result<FakeMachine> {
        let dir = tempfile::Builder::new().prefix("arch-sense-").tempdir()?;
        let machine = MachineRoot::new(dir.path());
        for step in self.steps {
            step(&machine)?;
        }
        Ok(FakeMachine { dir })
    }

    fn hwmon(self, index: usize, name: &str, files: &[(&str, &str)]) -> Self {
        let chip = move |m: &MachineRoot| m.hwmon_dir().join(format!("hwmon{index}"));
        let mut builder = self.file(move |m| chip(m).join("name"), name);
        for (file, value) in files {
            let file = file.to_string();
            builder = builder.file(move |m| chip(m).join(&file), *value);
        }
        builder
    }

    /// Removes `path` (file or folder) laid down by an earlier piece.
    pub fn without(mut self, path: impl FnOnce(&MachineRoot) -> PathBuf + 'static) -> Self {
        self.steps.push(Box::new(move |machine| {
            let path = path(machine);
            if path.is_dir() {
                fs::remove_dir_all(path)
            } else if path.exists() {
                fs::remove_file(path)
            } else {
                Ok(())
            }
        }));
        self
    }

    fn sense_interface(
        self,
        folder: fn(&MachineRoot) -> PathBuf,
        attrs: &[(&'static str, &'static str)],
    ) -> Self {
        let mut builder = self;
        for &(name, value) in attrs {
            builder = builder.file(move |m| folder(m).join(name), value);
        }
        builder
    }

    /// Makes `path` exist but fail every read, the way an attribute the firmware
    /// rejects does (ENODEV). A directory stands in for the file, so the read
    /// fails whoever runs the tests, root included.
    fn unreadable(mut self, path: impl FnOnce(&MachineRoot) -> PathBuf + 'static) -> Self {
        self.steps.push(Box::new(move |machine| {
            let path = path(machine);
            if path.is_file() {
                fs::remove_file(&path)?;
            }
            fs::create_dir_all(path)
        }));
        self
    }

    fn file(
        mut self,
        path: impl FnOnce(&MachineRoot) -> PathBuf + 'static,
        contents: impl Into<String>,
    ) -> Self {
        let contents = contents.into();
        self.steps.push(Box::new(move |machine| {
            let path = path(machine);
            if path.is_dir() {
                fs::remove_dir_all(&path)?; // an earlier `unreadable` stand-in
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, contents)
        }));
        self
    }
}
