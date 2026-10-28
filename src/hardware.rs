use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::{Context, Result, bail};

use crate::constants::{BRIGHT_HW_MAX, PREAMBLE, SPEED_HW_FAST, SPEED_HW_SLOW};
use crate::keyboard::{KeyboardTransport, Packet, UsbAccess, keyboard_access};
use crate::machine::MachineRoot;
use crate::models::{
    ControlChoice, ControlId, ControlItem, ControlKind, FanMode, OFF_EFFECT_INDEX,
    RANDOM_COLOR_INDEX, Rgb, RgbSettings, SensorMetric, SensorSnapshot,
};
use crate::permissions::setup_hint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SensorRole {
    Cpu,
    Gpu,
}

#[derive(Clone, Debug)]
struct HwmonFanSample {
    hwmon_name: String,
    label: Option<String>,
    rpm: u64,
    pwm: Option<u64>,
    pwm_max: Option<u64>,
}

#[derive(Clone, Debug)]
struct HwmonTempSample {
    hwmon_name: String,
    label: Option<String>,
    celsius: f64,
}

#[derive(Debug)]
pub(crate) enum HardwareRequest {
    Snapshot,
    ApplyControl { id: ControlId, value: String },
    ApplyRgb(RgbSettings),
    Shutdown,
}

#[derive(Debug)]
pub(crate) enum HardwareEvent {
    Snapshot(Box<HardwareSnapshot>),
    ControlApplied {
        id: ControlId,
        controls: Vec<ControlItem>,
    },
    ControlFailed {
        id: ControlId,
        error: String,
    },
    RgbApplied(String),
    RgbFailed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct HardwareSnapshot {
    pub(crate) module_loaded: bool,
    pub(crate) keyboard: UsbAccess,
    pub(crate) sensors: SensorSnapshot,
    pub(crate) controls: Vec<ControlItem>,
    pub(crate) note: Option<String>,
}

pub(crate) struct HardwareHandle {
    tx: Sender<HardwareRequest>,
    rx: Receiver<HardwareEvent>,
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
}

pub(crate) fn spawn_worker(
    machine: MachineRoot,
    keyboard: Box<dyn KeyboardTransport>,
) -> Result<HardwareHandle> {
    let (request_tx, request_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    thread::Builder::new()
        .name("arch-sense-hardware".into())
        .spawn(move || worker_loop(machine, keyboard, request_rx, event_tx))
        .context("starting hardware worker")?;

    Ok(HardwareHandle {
        tx: request_tx,
        rx: event_rx,
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
            HardwareRequest::ApplyControl { id, value } => match write_control(m, id, &value) {
                Ok(()) => HardwareEvent::ControlApplied {
                    id,
                    controls: load_controls(m),
                },
                Err(error) => HardwareEvent::ControlFailed {
                    id,
                    error: error.to_string(),
                },
            },
            HardwareRequest::ApplyRgb(settings) => {
                match apply_rgb_settings(&settings, keyboard.as_mut()) {
                    Ok(message) => HardwareEvent::RgbApplied(message),
                    Err(error) => HardwareEvent::RgbFailed(error.to_string()),
                }
            }
            HardwareRequest::Shutdown => break,
        };

        if tx.send(event).is_err() {
            break;
        }
    }
}

pub(crate) fn collect_snapshot(m: &MachineRoot) -> HardwareSnapshot {
    let module_loaded = m.predator_sense_dir().exists();
    let controls = load_controls(m);
    let sensors = read_sensors(m);
    let keyboard = keyboard_access(m);
    let note = hardware_note(m, module_loaded, &sensors);

    HardwareSnapshot {
        module_loaded,
        keyboard,
        sensors,
        controls,
        note,
    }
}

fn hardware_note(m: &MachineRoot, module_loaded: bool, sensors: &SensorSnapshot) -> Option<String> {
    if !module_loaded {
        return Some(format!(
            "linuwu_sense module offline: missing {}",
            m.predator_sense_dir().display()
        ));
    }

    [
        &sensors.cpu_temp,
        &sensors.gpu_temp,
        &sensors.cpu_fan,
        &sensors.gpu_fan,
    ]
    .iter()
    .find_map(|metric| metric.error.clone())
}

fn read_sensors(m: &MachineRoot) -> SensorSnapshot {
    let (cpu_fan, gpu_fan, cpu_fan_mode, gpu_fan_mode) = read_fan_telemetry(m);

    SensorSnapshot {
        cpu_temp: read_cpu_temp(m),
        gpu_temp: read_gpu_temp(m),
        cpu_fan,
        gpu_fan,
        cpu_fan_mode,
        gpu_fan_mode,
    }
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

    match read_gpu_temp_from_nvidia_smi() {
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

fn read_gpu_temp_from_nvidia_smi() -> Result<f64> {
    match Command::new("nvidia-smi")
        .args([
            "--query-gpu=temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
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

fn read_fan_telemetry(m: &MachineRoot) -> (SensorMetric, SensorMetric, FanMode, FanMode) {
    let linuwu_modes = read_linuwu_fan_modes(m);
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

fn read_linuwu_fan_modes(m: &MachineRoot) -> Option<(FanMode, FanMode)> {
    let raw = read_sysfs(&m.predator_sense_attr("fan_speed")).ok()?;
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
    let mut score = 0;

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
        .max_by_key(|(_, sample)| temperature_score(sample, role))
        .map(|(index, _)| index)
}

fn temperature_score(sample: &HwmonTempSample, role: SensorRole) -> i32 {
    let label = sample.label.as_deref().unwrap_or("");
    let haystack = format!("{} {label}", sample.hwmon_name).to_ascii_lowercase();
    let mut score = 0;

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

pub(crate) fn load_controls(m: &MachineRoot) -> Vec<ControlItem> {
    let thermal_choices = read_thermal_choices(m).unwrap_or_default();

    ControlId::ALL
        .iter()
        .copied()
        .map(|id| read_control(m, id, &thermal_choices))
        .collect()
}

fn read_control(m: &MachineRoot, id: ControlId, thermal_choices: &[String]) -> ControlItem {
    let kind = control_kind(id, thermal_choices);
    let raw_result = read_sysfs(&control_path(m, id));
    let (raw, last_error) = match raw_result {
        Ok(raw) => (raw, None),
        Err(error) => ("N/A".to_string(), Some(error.to_string())),
    };

    ControlItem {
        id,
        display: display_control_value(id, &raw),
        raw,
        kind,
        pending: None,
        last_error,
    }
}

fn read_thermal_choices(m: &MachineRoot) -> Result<Vec<String>> {
    Ok(read_sysfs(&m.platform_profile_choices())?
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect())
}

fn control_kind(id: ControlId, thermal_choices: &[String]) -> ControlKind {
    match id {
        ControlId::ThermalProfile => {
            let choices = if thermal_choices.is_empty() {
                vec![ControlChoice::new("N/A", "No profiles")]
            } else {
                thermal_choices
                    .iter()
                    .map(|choice| ControlChoice::new(choice, thermal_label(choice)))
                    .collect()
            };
            ControlKind::Choice(choices)
        }
        ControlId::FanSpeed => ControlKind::Choice(vec![
            ControlChoice::new("0,0", "Auto"),
            ControlChoice::new("100,100", "Max"),
        ]),
        ControlId::UsbCharging => ControlKind::Choice(vec![
            ControlChoice::new("0", "Off"),
            ControlChoice::new("10", "Until 10%"),
            ControlChoice::new("20", "Until 20%"),
            ControlChoice::new("30", "Until 30%"),
        ]),
        _ => ControlKind::Toggle,
    }
}

/// The file a control reads from and writes to.
fn control_path(m: &MachineRoot, id: ControlId) -> PathBuf {
    match id {
        ControlId::ThermalProfile => m.platform_profile(),
        ControlId::BacklightTimeout => m.predator_sense_attr("backlight_timeout"),
        ControlId::BatteryCalibration => m.predator_sense_attr("battery_calibration"),
        ControlId::BatteryLimiter => m.predator_sense_attr("battery_limiter"),
        ControlId::BootAnimation => m.predator_sense_attr("boot_animation_sound"),
        ControlId::FanSpeed => m.predator_sense_attr("fan_speed"),
        ControlId::LcdOverride => m.predator_sense_attr("lcd_override"),
        ControlId::UsbCharging => m.predator_sense_attr("usb_charging"),
    }
}

fn write_control(m: &MachineRoot, id: ControlId, value: &str) -> Result<()> {
    if value == "N/A" {
        bail!(
            "{} is unavailable because the hardware did not report choices",
            id.label()
        );
    }

    write_sysfs(&control_path(m, id), value)
}

fn display_control_value(id: ControlId, raw: &str) -> String {
    match id {
        ControlId::ThermalProfile => thermal_label(raw).to_string(),
        ControlId::BacklightTimeout | ControlId::BootAnimation | ControlId::LcdOverride => {
            on_off(raw)
        }
        ControlId::BatteryCalibration => match raw {
            "1" => "Running".to_string(),
            "0" => "Stopped".to_string(),
            other => other.to_string(),
        },
        ControlId::BatteryLimiter => match raw {
            "1" => "80% Limit".to_string(),
            "0" => "Disabled".to_string(),
            other => other.to_string(),
        },
        ControlId::FanSpeed => match raw {
            "0" | "0,0" => "Auto".to_string(),
            "100" | "100,100" => "Max".to_string(),
            other => format!("CPU/GPU {other}"),
        },
        ControlId::UsbCharging => match raw {
            "0" => "Disabled".to_string(),
            "10" => "Until 10%".to_string(),
            "20" => "Until 20%".to_string(),
            "30" => "Until 30%".to_string(),
            other => other.to_string(),
        },
    }
}

fn thermal_label(raw: &str) -> &str {
    match raw {
        "quiet" => "Quiet",
        "balanced" => "Balanced",
        "performance" => "Performance",
        "low-power" => "Low Power",
        other => other,
    }
}

fn on_off(raw: &str) -> String {
    match raw {
        "1" => "Enabled".to_string(),
        "0" => "Disabled".to_string(),
        other => other.to_string(),
    }
}

fn read_sysfs(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map(|content| content.trim().to_string())
        .map_err(|error| sysfs_error(error, "reading", path, None))
}

fn write_sysfs(path: &Path, value: &str) -> Result<()> {
    fs::write(path, value).map_err(|error| sysfs_error(error, "writing", path, Some(value)))
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

/// Sends the lighting in `settings` to the USB HID keyboard.
pub(crate) fn apply_rgb_settings(
    settings: &RgbSettings,
    keyboard: &mut dyn KeyboardTransport,
) -> Result<String> {
    keyboard.send(&rgb_packets(settings))?;
    Ok("Keyboard lighting applied".to_string())
}

fn rgb_packets(settings: &RgbSettings) -> Vec<Packet> {
    if settings.effect_idx == OFF_EFFECT_INDEX {
        return vec![PREAMBLE, [0x08, 0x02, 0x01, 0x00, 0x00, 0x01, 0x01, 0x9B]];
    }

    let mut packets = vec![PREAMBLE];
    if settings.effect().has_color && settings.color_idx != RANDOM_COLOR_INDEX {
        packets.push(make_color_packet(settings.color().rgb));
    }
    packets.push(make_effect_packet(settings));
    packets
}

fn make_color_packet(color: Rgb) -> Packet {
    [0x14, 0x00, 0x00, color.r, color.g, color.b, 0x00, 0x00]
}

fn make_effect_packet(settings: &RgbSettings) -> Packet {
    let effect = settings.effect();
    let hardware_brightness = ((settings.brightness as u16) * BRIGHT_HW_MAX as u16 / 100) as u8;
    let hardware_speed = if settings.speed >= 100 {
        SPEED_HW_FAST
    } else {
        let range = (SPEED_HW_SLOW - SPEED_HW_FAST) as u16;
        (SPEED_HW_SLOW - (settings.speed as u16 * range / 100) as u8).max(SPEED_HW_FAST)
    };
    let color_preset = if settings.color_idx == RANDOM_COLOR_INDEX {
        0x08
    } else {
        0x01
    };
    let direction = if effect.has_direction {
        settings.direction_idx as u8 + 1
    } else {
        0x01
    };

    [
        0x08,
        0x02,
        effect.opcode,
        hardware_speed,
        hardware_brightness,
        color_preset,
        direction,
        0x9B,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RgbConfig;
    use crate::models::RgbSettings;

    #[test]
    fn effect_packet_maps_brightness_and_speed_to_hardware_ranges() {
        let mut settings = RgbSettings::from_config(&RgbConfig::default());
        settings.brightness = 100;
        settings.speed = 0;

        let packet = make_effect_packet(&settings);

        assert_eq!(packet[3], SPEED_HW_SLOW);
        assert_eq!(packet[4], BRIGHT_HW_MAX);
    }

    #[test]
    fn display_values_are_human_readable() {
        assert_eq!(
            display_control_value(ControlId::ThermalProfile, "balanced"),
            "Balanced"
        );
        assert_eq!(display_control_value(ControlId::FanSpeed, "0,0"), "Auto");
        assert_eq!(
            display_control_value(ControlId::BatteryLimiter, "1"),
            "80% Limit"
        );
    }
}
