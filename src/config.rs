use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::permissions::setup_hint;

const CONFIG_DIR: &str = "/var/lib/arch-sense";
const CONFIG_FILE: &str = "config.json";

pub(crate) fn config_dir() -> PathBuf {
    PathBuf::from(CONFIG_DIR)
}

pub fn config_path() -> PathBuf {
    config_dir().join(CONFIG_FILE)
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
            brightness: 30,
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
        fs::create_dir_all(config_dir())
            .map_err(|e| config_error(e, "creating config directory"))?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(config_path(), json).map_err(|e| config_error(e, "writing config file"))?;
        Ok(())
    }
}

fn config_error(err: std::io::Error, action: &str) -> anyhow::Error {
    if err.kind() == ErrorKind::PermissionDenied {
        anyhow::anyhow!("{action} failed: {err}; {}", setup_hint())
    } else {
        anyhow::anyhow!("{action} failed: {err}")
    }
}
