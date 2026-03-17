use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub(crate) fn config_dir() -> PathBuf {
    // When running via sudo, save config in the real user's home
    let home = std::env::var("SUDO_USER")
        .ok()
        .map(|u| format!("/home/{u}"))
        .or_else(|| std::env::var("HOME").ok())
        .unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".config").join("arch-sense")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct RgbConfig {
    pub(crate) effect: usize,
    pub(crate) color: usize,
    pub(crate) brightness: u8,
    pub(crate) speed: u8,
    pub(crate) direction: usize,
}

impl Default for RgbConfig {
    fn default() -> Self {
        Self {
            effect: 1, // Static
            color: 9,  // White
            brightness: 80,
            speed: 50,
            direction: 0, // Right
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub(crate) rgb: RgbConfig,
}

impl AppConfig {
    pub fn load() -> Self {
        fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(&self) -> Result<()> {
        fs::create_dir_all(config_dir())?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(config_path(), json)?;
        Ok(())
    }
}
