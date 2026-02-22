use serde::{Deserialize, Serialize};
use shared::FanMode;
use std::fs;
use std::path::Path;

const CONFIG_DIR: &str = "/etc/arch-sense";
const CONFIG_FILE: &str = "/etc/arch-sense/config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub fan_mode: FanMode,
    pub battery_limiter: bool,
    pub keyboard_color: Option<(u8, u8, u8)>,
    pub keyboard_animation: Option<String>,
    #[serde(default = "default_keyboard_speed")]
    pub keyboard_speed: u8,
    #[serde(default = "default_keyboard_brightness")]
    pub keyboard_brightness: u8,
    pub lcd_overdrive: bool,
    pub boot_animation: bool,
    pub backlight_timeout: bool,
    pub usb_charging: u8,
}

fn default_keyboard_speed() -> u8 {
    5
}

fn default_keyboard_brightness() -> u8 {
    100
}

// 🌟 Default settings if the file doesn't exist yet
impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            fan_mode: FanMode::Auto,
            battery_limiter: false,
            keyboard_color: Some((0, 255, 255)), // Default Cyan
            keyboard_animation: None,
            keyboard_speed: default_keyboard_speed(),
            keyboard_brightness: default_keyboard_brightness(),
            lcd_overdrive: false,
            boot_animation: true,
            backlight_timeout: false,
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
