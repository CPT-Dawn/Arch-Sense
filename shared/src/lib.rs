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

// ==========================================
// THE COMMANDS (Client -> Daemon)
// ==========================================

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    GetHardwareStatus,
    SetFanMode(FanMode),
    SetBatteryLimiter(bool),
    SetKeyboardColor(u8, u8, u8),
    SetKeyboardAnimation(String),
    SetKeyboardSpeed(u8),
    SetKeyboardBrightness(u8),
    SetLcdOverdrive(bool),
    SetBootAnimation(bool),
    SetBacklightTimeout(bool),
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
        active_mode: String,
        battery_limiter: bool,
        lcd_overdrive: bool,
        boot_animation: bool,
        backlight_timeout: bool,
        usb_charging: u8,
        keyboard_color: Option<(u8, u8, u8)>,
        keyboard_animation: Option<String>,
        keyboard_speed: u8,
        keyboard_brightness: u8,
    },
}
