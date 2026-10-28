//! The machine root: the one place that knows where things live on a Linux box.
//!
//! Every hardware read and write resolves its path through a [`MachineRoot`]. The
//! production root is `/`; tests and demo mode point it at a fake tree in a temp
//! directory (see [`crate::fixture`]). No other module spells out an absolute path.

use std::path::{Path, PathBuf};

/// Where Linuwu-Sense hangs its attribute folders.
const LINUWU_SENSE_DIR: &str = "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi";
const PREDATOR_SENSE_FOLDER: &str = "predator_sense";
const NITRO_SENSE_FOLDER: &str = "nitro_sense";
const TURBO_STATE: &str = "turbo_sense/turbo_state";
const PLATFORM_PROFILE: &str = "/sys/firmware/acpi/platform_profile";
const PLATFORM_PROFILE_CHOICES: &str = "/sys/firmware/acpi/platform_profile_choices";
const HWMON_DIR: &str = "/sys/class/hwmon";
const CPU_THERMAL_ZONE_TEMP: &str = "/sys/class/thermal/thermal_zone0/temp";
const USB_DEVICES_DIR: &str = "/sys/bus/usb/devices";
const USB_DEVICE_NODES_DIR: &str = "/dev/bus/usb";
const STATE_DIR: &str = "/var/lib/arch-sense";
const PROC_SELF_STATUS: &str = "/proc/self/status";
const ETC_GROUP: &str = "/etc/group";
/// Where v1's pkexec self-installer put its files (v2 ships them in the package).
const UDEV_RULE: &str = "/etc/udev/rules.d/70-arch-sense.rules";
const PERMISSION_SERVICE: &str = "/etc/systemd/system/arch-sense-permissions.service";

/// A base directory under which all well-known system paths resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineRoot {
    root: PathBuf,
}

impl Default for MachineRoot {
    fn default() -> Self {
        Self::system()
    }
}

impl MachineRoot {
    /// The real machine: paths resolve exactly as written.
    pub fn system() -> Self {
        Self::new("/")
    }

    /// A machine whose filesystem starts at `root` (a fake tree for tests or demo mode).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn resolve(&self, absolute: &str) -> PathBuf {
        self.root.join(absolute.trim_start_matches('/'))
    }

    /// Linuwu-Sense's platform device folder, which holds one folder per Sense Interface.
    pub fn linuwu_sense_dir(&self) -> PathBuf {
        self.resolve(LINUWU_SENSE_DIR)
    }

    /// The Predator Sense attribute folder.
    pub fn predator_sense_dir(&self) -> PathBuf {
        self.linuwu_sense_dir().join(PREDATOR_SENSE_FOLDER)
    }

    /// One attribute file of the Predator Sense Interface, e.g. `fan_speed`.
    pub fn predator_sense_attr(&self, name: &str) -> PathBuf {
        self.predator_sense_dir().join(name)
    }

    /// The Nitro Sense attribute folder (legacy Nitro and Nitro v4 alike).
    pub fn nitro_sense_dir(&self) -> PathBuf {
        self.linuwu_sense_dir().join(NITRO_SENSE_FOLDER)
    }

    /// The read-only Turbo indicator (`turbo_sense/turbo_state`).
    pub fn turbo_state(&self) -> PathBuf {
        self.linuwu_sense_dir().join(TURBO_STATE)
    }

    /// The ACPI platform profile (the Thermal Profile).
    pub fn platform_profile(&self) -> PathBuf {
        self.resolve(PLATFORM_PROFILE)
    }

    /// The Thermal Profile choices the firmware offers.
    pub fn platform_profile_choices(&self) -> PathBuf {
        self.resolve(PLATFORM_PROFILE_CHOICES)
    }

    /// The hwmon class folder, one `hwmonN` subfolder per sensor chip.
    pub fn hwmon_dir(&self) -> PathBuf {
        self.resolve(HWMON_DIR)
    }

    /// The thermal-zone fallback for CPU temperature.
    pub fn cpu_thermal_zone_temp(&self) -> PathBuf {
        self.resolve(CPU_THERMAL_ZONE_TEMP)
    }

    /// The sysfs USB device folder, one subfolder per device and interface.
    pub fn usb_devices_dir(&self) -> PathBuf {
        self.resolve(USB_DEVICES_DIR)
    }

    /// The character device usbfs exposes for a USB device, e.g. `/dev/bus/usb/001/003`.
    pub fn usb_device_node(&self, bus: u16, device: u16) -> PathBuf {
        self.resolve(USB_DEVICE_NODES_DIR)
            .join(format!("{bus:03}"))
            .join(format!("{device:03}"))
    }

    /// Arch-Sense's own state folder, home of the Saved Lighting.
    pub fn state_dir(&self) -> PathBuf {
        self.resolve(STATE_DIR)
    }

    pub fn proc_self_status(&self) -> PathBuf {
        self.resolve(PROC_SELF_STATUS)
    }

    pub fn etc_group(&self) -> PathBuf {
        self.resolve(ETC_GROUP)
    }

    /// v1's self-installed udev rule, now a leftover the user may remove.
    pub fn udev_rule(&self) -> PathBuf {
        self.resolve(UDEV_RULE)
    }

    /// v1's self-installed permission service, now a leftover the user may remove.
    pub fn permission_service(&self) -> PathBuf {
        self.resolve(PERMISSION_SERVICE)
    }
}
