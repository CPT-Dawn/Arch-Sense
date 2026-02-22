use serde::{Deserialize, Serialize};

// ==========================================
// HARDWARE PROFILES
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FanMode {
    Auto,
    Quiet,
    Balanced,
    Performance,
    Turbo,
    Custom(u8, u8),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProfessionalColor {
    ArcticWhite,
    ArchCyan,
    NightShiftRed,
    EyeCareAmber,
}

impl ProfessionalColor {
    pub fn rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::ArcticWhite => (255, 255, 255),
            Self::ArchCyan => (0, 150, 255),
            Self::NightShiftRed => (255, 0, 0),
            Self::EyeCareAmber => (255, 150, 0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RgbMode {
    Solid(ProfessionalColor),
    Wave,
    Neon,
}

// ==========================================
// THE COMMANDS (Client -> Daemon)
// ==========================================

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    GetHardwareStatus,
    SetThermalProfile(String),
    SetFanMode(FanMode),
    SetBatteryLimiter(bool),
    SetRgbMode(RgbMode),
    IncreaseRgbBrightness,
    DecreaseRgbBrightness,
    ToggleSmartBatterySaver,
    SetLcdOverdrive(bool),
    SetBootAnimation(bool),
    SetUsbCharging(u8),
    SetBatteryCalibration(bool),
}

// ==========================================
// THE RESPONSES (Daemon -> Client)
// ==========================================

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ack(String),
    Error(String),
    HardwareStatus {
        cpu_temp: u8,
        gpu_temp: u8,
        cpu_fan_percent: u8,
        gpu_fan_percent: u8,
        thermal_profile: String,
        thermal_profile_choices: Vec<String>,
        fan_mode: FanMode,
        active_rgb_mode: RgbMode,
        rgb_brightness: u8,
        fx_speed: u8,
        smart_battery_saver: bool,
        battery_limiter: bool,
        battery_calibration: bool,
        lcd_overdrive: bool,
        boot_animation: bool,
        usb_charging: u8,
    },
}
