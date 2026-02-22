use serde::{Deserialize, Serialize};
use shared::{FanMode, ProfessionalColor, RgbMode};
use std::fs;
use std::path::Path;

const CONFIG_DIR: &str = "/etc/arch-sense";
const CONFIG_FILE: &str = "/etc/arch-sense/config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub fan_mode: FanMode,
    pub battery_limiter: bool,
    pub rgb_mode: RgbMode,
    pub rgb_brightness: u8,
    pub fx_speed: u8,
    pub lcd_overdrive: bool,
    pub boot_animation: bool,
    pub smart_battery_saver: bool,
    pub usb_charging: u8,
}

// 🌟 Default settings if the file doesn't exist yet
impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            fan_mode: FanMode::Auto,
            battery_limiter: false,
            rgb_mode: RgbMode::Solid(ProfessionalColor::ArchCyan),
            rgb_brightness: 70,
            fx_speed: 50,
            lcd_overdrive: false,
            boot_animation: true,
            smart_battery_saver: false,
            usb_charging: 0,
        }
    }
}

impl DaemonConfig {
    pub fn load() -> Self {
        if let Ok(content) = fs::read_to_string(CONFIG_FILE)
            && let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        Self::default()
    }

    pub fn save(&self) {
        if !Path::new(CONFIG_DIR).exists() {
            let _ = fs::create_dir_all(CONFIG_DIR);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(CONFIG_FILE, json);
        }
    }
}
