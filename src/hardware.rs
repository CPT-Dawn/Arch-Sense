use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, bail};

use crate::capabilities::{Capability, Laptop, detect};
use crate::constants::setup_hint;
use crate::keyboard::{KeyboardTransport, UsbAccess, keyboard_access};
use crate::lighting::{self, Lighting, LightingBackend, saved};
use crate::machine::MachineRoot;
use crate::models::{Battery, BatteryState, FanMode, SensorMetric, SensorSnapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SensorRole {
    Cpu,
    Gpu,
}

#[derive(Clone, Debug)]
struct HwmonFanSample {
    hwmon_name: String,
    index: usize,
    label: Option<String>,
    rpm: u64,
    pwm: Option<u64>,
    pwm_max: Option<u64>,
}

#[derive(Clone, Debug)]
struct HwmonTempSample {
    hwmon_name: String,
    index: usize,
    label: Option<String>,
    celsius: f64,
}

#[derive(Debug)]
pub(crate) enum HardwareRequest {
    Snapshot,
    /// Writes `value` to the file behind `capability`.
    Write {
        capability: Capability,
        value: String,
    },
    /// Sends `lighting` to the keyboard and saves it as the Saved Lighting.
    ApplyLighting(Lighting),
    Shutdown,
}

#[derive(Debug)]
pub(crate) enum HardwareEvent {
    Snapshot(Box<HardwareSnapshot>),
    Written {
        capability: Capability,
        value: String,
    },
    WriteFailed {
        capability: Capability,
        /// A short, human reason such as "permission denied".
        reason: String,
    },
    LightingApplied,
    /// The keyboard took the lighting but saving it failed.
    LightingNotSaved(String),
    LightingFailed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct HardwareSnapshot {
    pub(crate) laptop: Laptop,
    /// The laptop's model name from DMI, when it reads.
    pub(crate) model: Option<String>,
    pub(crate) keyboard: UsbAccess,
    /// The keyboard's Lighting Backend, if it has one Arch-Sense drives.
    pub(crate) lighting: Option<LightingBackend>,
    pub(crate) sensors: SensorSnapshot,
    /// The current raw value of every Capability present.
    pub(crate) values: BTreeMap<Capability, String>,
}

pub(crate) struct HardwareHandle {
    tx: Sender<HardwareRequest>,
    rx: Receiver<HardwareEvent>,
    worker: Option<JoinHandle<()>>,
}

impl HardwareHandle {
    pub(crate) fn send(&self, request: HardwareRequest) -> Result<()> {
        self.tx
            .send(request)
            .context("hardware worker is not available")
    }

    pub(crate) fn drain(&self) -> Vec<HardwareEvent> {
        self.rx.try_iter().collect()
    }

    /// Stops the worker once it has finished every request already sent, and waits for it.
    pub(crate) fn shutdown(&mut self) {
        let _ = self.tx.send(HardwareRequest::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(crate) fn spawn_worker(
    machine: MachineRoot,
    keyboard: Box<dyn KeyboardTransport>,
) -> Result<HardwareHandle> {
    let (request_tx, request_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    let worker = thread::Builder::new()
        .name("arch-sense-hardware".into())
        .spawn(move || worker_loop(machine, keyboard, request_rx, event_tx))
        .context("starting hardware worker")?;

    Ok(HardwareHandle {
        tx: request_tx,
        rx: event_rx,
        worker: Some(worker),
    })
}

fn worker_loop(
    machine: MachineRoot,
    mut keyboard: Box<dyn KeyboardTransport>,
    rx: Receiver<HardwareRequest>,
    tx: Sender<HardwareEvent>,
) {
    let m = &machine;
    for request in rx {
        let event = match request {
            HardwareRequest::Snapshot => HardwareEvent::Snapshot(Box::new(collect_snapshot(m))),
            HardwareRequest::Write { capability, value } => {
                match write_capability(m, capability, &value) {
                    Ok(()) => HardwareEvent::Written { capability, value },
                    Err(reason) => HardwareEvent::WriteFailed { capability, reason },
                }
            }
            HardwareRequest::ApplyLighting(lighting) => {
                apply_lighting(m, &lighting, keyboard.as_mut())
            }
            HardwareRequest::Shutdown => break,
        };

        if tx.send(event).is_err() {
            break;
        }
    }
}

fn apply_lighting(
    m: &MachineRoot,
    lighting: &Lighting,
    keyboard: &mut dyn KeyboardTransport,
) -> HardwareEvent {
    if let Err(error) = lighting::apply(lighting, m, keyboard) {
        return HardwareEvent::LightingFailed(error.to_string());
    }
    match saved::save(m, lighting) {
        Ok(()) => HardwareEvent::LightingApplied,
        Err(error) => HardwareEvent::LightingNotSaved(error.to_string()),
    }
}

pub(crate) fn collect_snapshot(m: &MachineRoot) -> HardwareSnapshot {
    let laptop = detect(m);
    let values = laptop
        .capabilities()
        .iter()
        .filter_map(|&capability| {
            let path = capability.path(m, laptop.sense_interface)?;
            read_sysfs(&path).ok().map(|value| (capability, value))
        })
        .collect();
    let sensors = read_sensors(m, &laptop);
    let keyboard = keyboard_access(m);
    let lighting = lighting::detect(m);
    let model = read_optional_string(&m.dmi_product_name());

    HardwareSnapshot {
        laptop,
        model,
        keyboard,
        lighting,
        sensors,
        values,
    }
}

/// Writes `value` to `capability`'s file, re-detecting the laptop first so a
/// Capability that vanished is refused rather than written somewhere stale.
fn write_capability(m: &MachineRoot, capability: Capability, value: &str) -> Result<(), String> {
    let laptop = detect(m);
    let path = Some(&laptop)
        .filter(|laptop| laptop.has(capability))
        .and_then(|laptop| capability.path(m, laptop.sense_interface))
        .ok_or_else(|| "not available on this laptop".to_string())?;
    fs::write(path, value).map_err(|error| match error.kind() {
        ErrorKind::PermissionDenied => "permission denied".to_string(),
        ErrorKind::NotFound => "attribute not found".to_string(),
        _ => error.to_string(),
    })
}

fn read_sensors(m: &MachineRoot, laptop: &Laptop) -> SensorSnapshot {
    let (cpu_fan, gpu_fan, cpu_fan_mode, gpu_fan_mode) = read_fan_telemetry(m, laptop);

    SensorSnapshot {
        cpu_temp: read_cpu_temp(m),
        gpu_temp: read_gpu_temp(m),
        cpu_fan,
        gpu_fan,
        cpu_fan_mode,
        gpu_fan_mode,
        battery: read_battery(m),
    }
}

/// The first battery in the power-supply class, if the laptop has one that reads.
fn read_battery(m: &MachineRoot) -> Option<Battery> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(m.power_supply_dir())
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| read_optional_string(&dir.join("type")).as_deref() == Some("Battery"))
        .collect();
    dirs.sort();
    let dir = dirs.into_iter().next()?;

    let percent = read_optional_u64(&dir.join("capacity"))?.min(100) as u8;
    let state = match read_optional_string(&dir.join("status")).as_deref() {
        Some("Charging") => BatteryState::Charging,
        Some("Discharging") => BatteryState::Discharging,
        Some("Full") => BatteryState::Full,
        Some("Not charging") => BatteryState::NotCharging,
        _ => BatteryState::Unknown,
    };
    let micro = |name: &str| read_optional_u64(&dir.join(name)).map(|value| value as f64);
    let watts = micro("power_now")
        .or_else(|| Some(micro("current_now")? * micro("voltage_now")? / 1e6))
        .map(|microwatts| microwatts / 1e6);

    Some(Battery {
        percent,
        state,
        watts,
    })
}

fn read_cpu_temp(m: &MachineRoot) -> SensorMetric {
    let hwmon = read_hwmon_temperature(m, SensorRole::Cpu);
    if let Ok(value) = hwmon {
        return SensorMetric::available(value);
    }

    let hwmon_error = hwmon.err().map(|error| error.to_string());

    let zone = m.cpu_thermal_zone_temp();
    match read_sysfs(&zone).and_then(|raw| {
        raw.parse::<f64>()
            .map(|value| value / 1000.0)
            .with_context(|| format!("parsing CPU temperature from {}: {raw}", zone.display()))
    }) {
        Ok(value) => SensorMetric::available(value),
        Err(error) => {
            let detail = match hwmon_error {
                Some(hwmon_error) => {
                    format!("hwmon: {hwmon_error}; thermal zone: {error}")
                }
                None => error.to_string(),
            };
            SensorMetric::unavailable(format!("CPU temperature unavailable: {detail}"))
        }
    }
}

fn read_gpu_temp(m: &MachineRoot) -> SensorMetric {
    let hwmon = read_hwmon_temperature(m, SensorRole::Gpu);
    if let Ok(value) = hwmon {
        return SensorMetric::available(value);
    }

    let hwmon_error = hwmon.err().map(|error| error.to_string());

    match read_gpu_temp_from_nvidia_smi(m) {
        Ok(value) => SensorMetric::available(value),
        Err(error) => {
            let detail = match hwmon_error {
                Some(hwmon_error) => format!("hwmon: {hwmon_error}; nvidia-smi: {error}"),
                None => error.to_string(),
            };
            SensorMetric::unavailable(format!("GPU temperature unavailable: {detail}"))
        }
    }
}

fn read_gpu_temp_from_nvidia_smi(m: &MachineRoot) -> Result<f64> {
    match Command::new(m.nvidia_smi())
        .args([
            "--query-gpu=temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // One line per GPU; the first is the one the card shows.
            let raw = stdout.lines().next().unwrap_or_default().trim().to_string();
            raw.parse::<f64>()
                .with_context(|| format!("parsing GPU temperature from nvidia-smi output '{raw}'"))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() {
                format!("nvidia-smi exited with {}", output.status)
            } else {
                format!("nvidia-smi failed: {stderr}")
            };
            bail!("{detail}")
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            bail!("nvidia-smi is not installed")
        }
        Err(error) => bail!("starting nvidia-smi failed: {error}"),
    }
}

fn read_fan_telemetry(
    m: &MachineRoot,
    laptop: &Laptop,
) -> (SensorMetric, SensorMetric, FanMode, FanMode) {
    let linuwu_modes = read_linuwu_fan_modes(m, laptop);
    let samples = match collect_hwmon_fan_samples(m) {
        Ok(samples) => samples,
        Err(error) => {
            let message = format!("hwmon fan discovery failed: {error}");
            return (
                SensorMetric::unavailable(format!("CPU fan RPM unavailable: {message}")),
                SensorMetric::unavailable(format!("GPU fan RPM unavailable: {message}")),
                linuwu_modes.map(|(cpu, _)| cpu).unwrap_or(FanMode::Auto),
                linuwu_modes.map(|(_, gpu)| gpu).unwrap_or(FanMode::Auto),
            );
        }
    };

    let (cpu_idx, gpu_idx) = select_fan_sample_indices(&samples);
    let hwmon_dir = m.hwmon_dir();

    let cpu_fan = cpu_idx
        .and_then(|index| samples.get(index))
        .map(|sample| SensorMetric::available(sample.rpm as f64))
        .unwrap_or_else(|| {
            SensorMetric::unavailable(format!(
                "CPU fan RPM unavailable: no matching fan*_input under {}",
                hwmon_dir.display()
            ))
        });

    let gpu_fan = gpu_idx
        .and_then(|index| samples.get(index))
        .map(|sample| SensorMetric::available(sample.rpm as f64))
        .unwrap_or_else(|| {
            SensorMetric::unavailable(format!(
                "GPU fan RPM unavailable: no matching fan*_input under {}",
                hwmon_dir.display()
            ))
        });

    let cpu_mode = linuwu_modes
        .map(|(cpu, _)| cpu)
        .or_else(|| {
            cpu_idx
                .and_then(|index| samples.get(index))
                .map(mode_from_hwmon_sample)
        })
        .unwrap_or(FanMode::Auto);

    let gpu_mode = linuwu_modes
        .map(|(_, gpu)| gpu)
        .or_else(|| {
            gpu_idx
                .and_then(|index| samples.get(index))
                .map(mode_from_hwmon_sample)
        })
        .unwrap_or(FanMode::Auto);

    (cpu_fan, gpu_fan, cpu_mode, gpu_mode)
}

fn read_linuwu_fan_modes(m: &MachineRoot, laptop: &Laptop) -> Option<(FanMode, FanMode)> {
    let path = laptop.sense_interface.attribute(m, "fan_speed")?;
    let raw = read_sysfs(&path).ok()?;
    let parts: Vec<&str> = raw.split(',').collect();

    let parse_mode = |index: usize| -> Option<FanMode> {
        let value = parts.get(index)?.trim().parse::<f64>().ok()?;
        Some(if value >= 100.0 {
            FanMode::Max
        } else {
            FanMode::Auto
        })
    };

    Some((parse_mode(0)?, parse_mode(1)?))
}

fn select_fan_sample_indices(samples: &[HwmonFanSample]) -> (Option<usize>, Option<usize>) {
    if samples.is_empty() {
        return (None, None);
    }

    let cpu = best_fan_index(samples, SensorRole::Cpu, None).or(Some(0));
    let gpu = best_fan_index(samples, SensorRole::Gpu, cpu).or_else(|| {
        samples
            .iter()
            .enumerate()
            .find(|(index, _)| Some(*index) != cpu)
            .map(|(index, _)| index)
    });

    (cpu, gpu)
}

fn best_fan_index(
    samples: &[HwmonFanSample],
    role: SensorRole,
    exclude: Option<usize>,
) -> Option<usize> {
    samples
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != exclude)
        .max_by_key(|(_, sample)| (fan_score(sample, role), sample.rpm))
        .map(|(index, _)| index)
}

fn fan_score(sample: &HwmonFanSample, role: SensorRole) -> i32 {
    let label = sample.label.as_deref().unwrap_or("");
    let haystack = format!("{} {label}", sample.hwmon_name).to_ascii_lowercase();
    let mut score = acer_channel_score(&sample.hwmon_name, sample.index, role);

    if contains_any(&haystack, role_keywords(role)) {
        score += 6;
    }

    match role {
        SensorRole::Cpu => {
            if contains_any(&haystack, &["package", "tctl", "tdie", "coretemp", "cpu"]) {
                score += 4;
            }
            if contains_any(&haystack, &["gpu", "amdgpu", "nouveau", "nvidia"]) {
                score -= 4;
            }
        }
        SensorRole::Gpu => {
            if contains_any(&haystack, &["gpu", "edge", "junction", "amdgpu", "nvidia"]) {
                score += 4;
            }
            if contains_any(&haystack, &["cpu", "package", "coretemp"]) {
                score -= 4;
            }
        }
    }

    score
}

fn mode_from_hwmon_sample(sample: &HwmonFanSample) -> FanMode {
    if let Some(pwm) = sample.pwm {
        let pwm_max = sample.pwm_max.unwrap_or(255);
        if pwm_max > 0 && pwm >= pwm_max.saturating_sub(1) {
            return FanMode::Max;
        }
    }

    FanMode::Auto
}

fn collect_hwmon_fan_samples(m: &MachineRoot) -> Result<Vec<HwmonFanSample>> {
    let mut samples = Vec::new();

    for hwmon_dir in list_hwmon_dirs(m)? {
        let hwmon_name =
            read_optional_string(&hwmon_dir.join("name")).unwrap_or_else(|| "unknown".to_string());
        let entries = match fs::read_dir(&hwmon_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            let Some(index) = parse_indexed_attr(&file_name, "fan", "_input") else {
                continue;
            };

            let fan_path = hwmon_dir.join(format!("fan{index}_input"));
            let Some(rpm) = read_optional_u64(&fan_path) else {
                continue;
            };

            let label = read_optional_string(&hwmon_dir.join(format!("fan{index}_label")));
            let pwm = read_optional_u64(&hwmon_dir.join(format!("pwm{index}")));
            let pwm_max = read_optional_u64(&hwmon_dir.join(format!("pwm{index}_max")))
                .or(if pwm.is_some() { Some(255) } else { None });

            samples.push(HwmonFanSample {
                hwmon_name: hwmon_name.clone(),
                index,
                label,
                rpm,
                pwm,
                pwm_max,
            });
        }
    }

    Ok(samples)
}

fn read_hwmon_temperature(m: &MachineRoot, role: SensorRole) -> Result<f64> {
    let samples = collect_hwmon_temp_samples(m)?;
    let Some(index) = best_temp_index(&samples, role) else {
        bail!(
            "no temp*_input match for {} role in {}",
            match role {
                SensorRole::Cpu => "CPU",
                SensorRole::Gpu => "GPU",
            },
            m.hwmon_dir().display()
        );
    };

    Ok(samples[index].celsius)
}

fn collect_hwmon_temp_samples(m: &MachineRoot) -> Result<Vec<HwmonTempSample>> {
    let mut samples = Vec::new();

    for hwmon_dir in list_hwmon_dirs(m)? {
        let hwmon_name =
            read_optional_string(&hwmon_dir.join("name")).unwrap_or_else(|| "unknown".to_string());
        let entries = match fs::read_dir(&hwmon_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            let Some(index) = parse_indexed_attr(&file_name, "temp", "_input") else {
                continue;
            };

            let temp_path = hwmon_dir.join(format!("temp{index}_input"));
            let Some(raw) = read_optional_string(&temp_path) else {
                continue;
            };
            let Ok(raw_value) = raw.parse::<f64>() else {
                continue;
            };

            let celsius = if raw_value.abs() > 1000.0 {
                raw_value / 1000.0
            } else {
                raw_value
            };

            if !(-40.0..=130.0).contains(&celsius) {
                continue;
            }

            let label = read_optional_string(&hwmon_dir.join(format!("temp{index}_label")));
            samples.push(HwmonTempSample {
                hwmon_name: hwmon_name.clone(),
                index,
                label,
                celsius,
            });
        }
    }

    Ok(samples)
}

fn best_temp_index(samples: &[HwmonTempSample], role: SensorRole) -> Option<usize> {
    samples
        .iter()
        .enumerate()
        .map(|(index, sample)| (index, temperature_score(sample, role)))
        .filter(|&(_, score)| score > 0)
        .max_by_key(|&(_, score)| score)
        .map(|(index, _)| index)
}

fn temperature_score(sample: &HwmonTempSample, role: SensorRole) -> i32 {
    let label = sample.label.as_deref().unwrap_or("");
    let haystack = format!("{} {label}", sample.hwmon_name).to_ascii_lowercase();
    let mut score = acer_channel_score(&sample.hwmon_name, sample.index, role);

    if contains_any(&haystack, role_keywords(role)) {
        score += 6;
    }

    match role {
        SensorRole::Cpu => {
            if contains_any(&haystack, &["package", "tctl", "tdie", "coretemp", "cpu"]) {
                score += 4;
            }
            if contains_any(&haystack, &["gpu", "amdgpu", "nouveau", "nvidia"]) {
                score -= 4;
            }
        }
        SensorRole::Gpu => {
            if contains_any(&haystack, &["gpu", "edge", "junction", "amdgpu", "nvidia"]) {
                score += 4;
            }
            if contains_any(&haystack, &["cpu", "package", "coretemp"]) {
                score -= 4;
            }
        }
    }

    score
}

/// Linuwu-Sense's `acer` hwmon chip has no labels: channel 1 is the CPU and
/// channel 2 the GPU, for temperatures and fans alike. The bonus stays below a
/// keyword match, so a labelled `coretemp` or `amdgpu` still wins.
fn acer_channel_score(hwmon_name: &str, index: usize, role: SensorRole) -> i32 {
    let channel = match role {
        SensorRole::Cpu => 1,
        SensorRole::Gpu => 2,
    };
    if hwmon_name == "acer" && index == channel {
        5
    } else {
        0
    }
}

fn role_keywords(role: SensorRole) -> &'static [&'static str] {
    match role {
        SensorRole::Cpu => &["cpu", "coretemp", "k10temp", "x86_pkg_temp", "acpitz"],
        SensorRole::Gpu => &["gpu", "amdgpu", "nouveau", "nvidia", "radeon"],
    }
}

fn contains_any(haystack: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| haystack.contains(keyword))
}

fn read_optional_string(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_optional_u64(path: &Path) -> Option<u64> {
    read_optional_string(path).and_then(|value| value.parse::<u64>().ok())
}

fn parse_indexed_attr(name: &str, prefix: &str, suffix: &str) -> Option<usize> {
    if !name.starts_with(prefix) || !name.ends_with(suffix) {
        return None;
    }

    let start = prefix.len();
    let end = name.len().saturating_sub(suffix.len());
    if start >= end {
        return None;
    }

    name[start..end].parse::<usize>().ok()
}

fn list_hwmon_dirs(m: &MachineRoot) -> Result<Vec<PathBuf>> {
    let hwmon_dir = m.hwmon_dir();
    let mut dirs = Vec::new();

    for entry in
        fs::read_dir(&hwmon_dir).with_context(|| format!("reading {}", hwmon_dir.display()))?
    {
        let entry = entry.with_context(|| format!("reading entries in {}", hwmon_dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    dirs.sort();
    Ok(dirs)
}

fn read_sysfs(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map(|content| content.trim().to_string())
        .map_err(|error| sysfs_error(error, "reading", path, None))
}

fn sysfs_error(
    error: std::io::Error,
    action: &str,
    path: &Path,
    value: Option<&str>,
) -> anyhow::Error {
    let path = path.display();
    let target = value
        .map(|value| format!(" value '{value}' to {path}"))
        .unwrap_or_else(|| format!(" {path}"));

    if error.kind() == ErrorKind::PermissionDenied {
        anyhow::anyhow!("{action}{target} failed: {error}; {}", setup_hint())
    } else {
        anyhow::anyhow!("{action}{target} failed: {error}")
    }
}
