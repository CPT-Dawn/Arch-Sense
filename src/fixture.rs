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
        const ATTRS: [(&str, &str); 7] = [
            ("backlight_timeout", "0"),
            ("battery_calibration", "0"),
            ("battery_limiter", "1"),
            ("boot_animation_sound", "0"),
            ("fan_speed", "0,0"),
            ("lcd_override", "0"),
            ("usb_charging", "0"),
        ];

        let mut builder = self;
        for (name, value) in ATTRS {
            builder = builder.file(move |m| m.predator_sense_attr(name), value);
        }
        builder
            .file(MachineRoot::platform_profile, "balanced")
            .file(
                MachineRoot::platform_profile_choices,
                "quiet balanced performance",
            )
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

    fn file(
        mut self,
        path: impl FnOnce(&MachineRoot) -> PathBuf + 'static,
        contents: impl Into<String>,
    ) -> Self {
        let contents = contents.into();
        self.steps.push(Box::new(move |machine| {
            let path = path(machine);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, contents)
        }));
        self
    }
}
