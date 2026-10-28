use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::machine::MachineRoot;
use crate::permissions::setup_hint;

const CONFIG_FILE: &str = "config.json";

pub fn config_path(machine: &MachineRoot) -> PathBuf {
    machine.state_dir().join(CONFIG_FILE)
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
    pub fn load(machine: &MachineRoot) -> Self {
        Self::load_with_warning(machine).0
    }

    pub(crate) fn load_with_warning(machine: &MachineRoot) -> (Self, Option<String>) {
        let path = config_path(machine);
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(config) => (config, None),
                Err(error) => (
                    Self::default(),
                    Some(format!(
                        "Config parse failed at {}: {error}; using defaults",
                        path.display()
                    )),
                ),
            },
            Err(error) if error.kind() == ErrorKind::NotFound => (Self::default(), None),
            Err(error) => (
                Self::default(),
                Some(format!(
                    "Config read failed at {}: {error}; using defaults",
                    path.display()
                )),
            ),
        }
    }

    pub(crate) fn save(&self, machine: &MachineRoot) -> Result<()> {
        fs::create_dir_all(machine.state_dir())
            .map_err(|e| config_error(e, "creating config directory"))?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(config_path(machine), json)
            .map_err(|e| config_error(e, "writing config file"))?;
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
