use std::time::Duration;

pub(crate) const PS_BASE: &str =
    "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense";
pub(crate) const PLATFORM_PROFILE: &str = "/sys/firmware/acpi/platform_profile";
pub(crate) const PROFILE_CHOICES: &str = "/sys/firmware/acpi/platform_profile_choices";
pub(crate) const CPU_TEMP_PATH: &str = "/sys/class/thermal/thermal_zone0/temp";

// USB keyboard (Acer Predator PH16-71)
pub(crate) const KB_VID: u16 = 0x04F2;
pub(crate) const KB_PID: u16 = 0x0117;
pub(crate) const KB_IFACE: u8 = 3;
pub(crate) const KB_EP: u8 = 0x04;
pub(crate) const USB_TIMEOUT: Duration = Duration::from_millis(1000);

// RGB protocol limits
pub(crate) const BRIGHT_HW_MAX: u8 = 50; // 0x32
pub(crate) const SPEED_HW_FAST: u8 = 1;
pub(crate) const SPEED_HW_SLOW: u8 = 9;
pub(crate) const PREAMBLE: [u8; 8] = [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E];

pub(crate) fn ps(name: &str) -> String {
    format!("{PS_BASE}/{name}")
}
