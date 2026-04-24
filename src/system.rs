use std::fs;
use std::io::ErrorKind;
use std::process::Command;

use anyhow::Result;

use crate::constants::{ps, CPU_TEMP_PATH, PROFILE_CHOICES};
use crate::permissions::setup_hint;

pub(crate) fn sysfs_read(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub(crate) fn sysfs_write(path: &str, val: &str) -> Result<()> {
    fs::write(path, val).map_err(|e| {
        if e.kind() == ErrorKind::PermissionDenied {
            anyhow::anyhow!("{e} - writing '{val}' to {path}; {}", setup_hint())
        } else {
            anyhow::anyhow!("{e} - writing '{val}' to {path}")
        }
    })
}

pub(crate) fn cpu_temp() -> Option<f64> {
    sysfs_read(CPU_TEMP_PATH)?
        .parse::<f64>()
        .ok()
        .map(|t| t / 1000.0)
}

pub(crate) fn gpu_temp() -> Option<f64> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if out.status.success() {
        String::from_utf8(out.stdout).ok()?.trim().parse().ok()
    } else {
        None
    }
}

pub(crate) fn fan_speeds() -> (Option<u32>, Option<u32>) {
    let s = match sysfs_read(&ps("fan_speed")) {
        Some(s) => s,
        None => return (None, None),
    };
    let p: Vec<&str> = s.split(',').collect();
    (
        p.first().and_then(|v| v.trim().parse().ok()),
        p.get(1).and_then(|v| v.trim().parse().ok()),
    )
}

pub(crate) fn thermal_choices() -> Vec<String> {
    sysfs_read(PROFILE_CHOICES)
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default()
}
