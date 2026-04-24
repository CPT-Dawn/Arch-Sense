use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::{bail, Context, Result};

use crate::constants::{
    ps, BRIGHT_HW_MAX, CPU_TEMP_PATH, KB_EP, KB_IFACE, PLATFORM_PROFILE, PREAMBLE, PROFILE_CHOICES,
    PS_BASE, SPEED_HW_FAST, SPEED_HW_SLOW, USB_TIMEOUT,
};
use crate::models::{
    ControlChoice, ControlId, ControlItem, ControlKind, Rgb, RgbSettings, SensorMetric,
    SensorSnapshot, OFF_EFFECT_INDEX, RANDOM_COLOR_INDEX,
};
use crate::permissions::{keyboard_access, keyboard_present, open_keyboard, setup_hint, UsbAccess};

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

pub(crate) fn spawn_worker() -> Result<HardwareHandle> {
    let (request_tx, request_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    thread::Builder::new()
        .name("arch-sense-hardware".into())
        .spawn(move || worker_loop(request_rx, event_tx))
        .context("starting hardware worker")?;

    Ok(HardwareHandle {
        tx: request_tx,
        rx: event_rx,
    })
}

fn worker_loop(rx: Receiver<HardwareRequest>, tx: Sender<HardwareEvent>) {
    for request in rx {
        let event = match request {
            HardwareRequest::Snapshot => HardwareEvent::Snapshot(Box::new(collect_snapshot())),
            HardwareRequest::ApplyControl { id, value } => match write_control(id, &value) {
                Ok(()) => HardwareEvent::ControlApplied {
                    id,
                    controls: load_controls(),
                },
                Err(error) => HardwareEvent::ControlFailed {
                    id,
                    error: error.to_string(),
                },
            },
            HardwareRequest::ApplyRgb(settings) => match apply_rgb_settings(&settings) {
                Ok(message) => HardwareEvent::RgbApplied(message),
                Err(error) => HardwareEvent::RgbFailed(error.to_string()),
            },
            HardwareRequest::Shutdown => break,
        };

        if tx.send(event).is_err() {
            break;
        }
    }
}

pub(crate) fn collect_snapshot() -> HardwareSnapshot {
    let module_loaded = Path::new(PS_BASE).exists();
    let controls = load_controls();
    let sensors = read_sensors();
    let keyboard = keyboard_access();
    let note = hardware_note(module_loaded, &sensors);

    HardwareSnapshot {
        module_loaded,
        keyboard,
        sensors,
        controls,
        note,
    }
}

fn hardware_note(module_loaded: bool, sensors: &SensorSnapshot) -> Option<String> {
    if !module_loaded {
        return Some(format!("linuwu_sense module offline: missing {PS_BASE}"));
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

fn read_sensors() -> SensorSnapshot {
    let (cpu_fan, gpu_fan) = read_fan_speeds();

    SensorSnapshot {
        cpu_temp: read_cpu_temp(),
        gpu_temp: read_gpu_temp(),
        cpu_fan,
        gpu_fan,
    }
}

fn read_cpu_temp() -> SensorMetric {
    match read_sysfs(CPU_TEMP_PATH).and_then(|raw| {
        raw.parse::<f64>()
            .map(|value| value / 1000.0)
            .with_context(|| format!("parsing CPU temperature from {CPU_TEMP_PATH}: {raw}"))
    }) {
        Ok(value) => SensorMetric::available(value),
        Err(error) => SensorMetric::unavailable(format!("CPU temperature unavailable: {error}")),
    }
}

fn read_gpu_temp() -> SensorMetric {
    match Command::new("nvidia-smi")
        .args([
            "--query-gpu=temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            match raw.parse::<f64>() {
                Ok(value) => SensorMetric::available(value),
                Err(error) => SensorMetric::unavailable(format!(
                    "GPU temperature parse failed from nvidia-smi output '{raw}': {error}"
                )),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() {
                format!("nvidia-smi exited with {}", output.status)
            } else {
                format!("nvidia-smi failed: {stderr}")
            };
            SensorMetric::unavailable(format!("GPU temperature unavailable: {detail}"))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            SensorMetric::unavailable("GPU temperature unavailable: nvidia-smi not installed")
        }
        Err(error) => SensorMetric::unavailable(format!(
            "GPU temperature unavailable: starting nvidia-smi failed: {error}"
        )),
    }
}

fn read_fan_speeds() -> (SensorMetric, SensorMetric) {
    let path = ps("fan_speed");
    match read_sysfs(&path) {
        Ok(raw) => {
            let parts: Vec<&str> = raw.split(',').collect();
            let parse = |index: usize, label: &str| {
                parts
                    .get(index)
                    .ok_or_else(|| anyhow::anyhow!("{label} fan value missing in '{raw}'"))
                    .and_then(|part| {
                        part.trim()
                            .parse::<f64>()
                            .with_context(|| format!("parsing {label} fan percentage from '{raw}'"))
                    })
            };

            let cpu = parse(0, "CPU")
                .map(SensorMetric::available)
                .unwrap_or_else(|error| SensorMetric::unavailable(error.to_string()));
            let gpu = parse(1, "GPU")
                .map(SensorMetric::available)
                .unwrap_or_else(|error| SensorMetric::unavailable(error.to_string()));
            (cpu, gpu)
        }
        Err(error) => {
            let message = format!("fan telemetry unavailable: {error}");
            (
                SensorMetric::unavailable(message.clone()),
                SensorMetric::unavailable(message),
            )
        }
    }
}

pub(crate) fn load_controls() -> Vec<ControlItem> {
    let thermal_choices = read_thermal_choices().unwrap_or_default();

    ControlId::ALL
        .iter()
        .copied()
        .map(|id| read_control(id, &thermal_choices))
        .collect()
}

fn read_control(id: ControlId, thermal_choices: &[String]) -> ControlItem {
    let kind = control_kind(id, thermal_choices);
    let raw_result = read_control_raw(id);
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

fn read_thermal_choices() -> Result<Vec<String>> {
    Ok(read_sysfs(PROFILE_CHOICES)?
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
            ControlChoice::new("30,30", "Low 30%"),
            ControlChoice::new("50,50", "Medium 50%"),
            ControlChoice::new("70,70", "High 70%"),
            ControlChoice::new("100,100", "Maximum"),
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

fn read_control_raw(id: ControlId) -> Result<String> {
    match id {
        ControlId::ThermalProfile => read_sysfs(PLATFORM_PROFILE),
        ControlId::BacklightTimeout => read_sysfs(&ps("backlight_timeout")),
        ControlId::BatteryCalibration => read_sysfs(&ps("battery_calibration")),
        ControlId::BatteryLimiter => read_sysfs(&ps("battery_limiter")),
        ControlId::BootAnimation => read_sysfs(&ps("boot_animation_sound")),
        ControlId::FanSpeed => read_sysfs(&ps("fan_speed")),
        ControlId::LcdOverride => read_sysfs(&ps("lcd_override")),
        ControlId::UsbCharging => read_sysfs(&ps("usb_charging")),
    }
}

fn write_control(id: ControlId, value: &str) -> Result<()> {
    if value == "N/A" {
        bail!(
            "{} is unavailable because the hardware did not report choices",
            id.label()
        );
    }

    match id {
        ControlId::ThermalProfile => write_sysfs(PLATFORM_PROFILE, value),
        ControlId::BacklightTimeout => write_sysfs(&ps("backlight_timeout"), value),
        ControlId::BatteryCalibration => write_sysfs(&ps("battery_calibration"), value),
        ControlId::BatteryLimiter => write_sysfs(&ps("battery_limiter"), value),
        ControlId::BootAnimation => write_sysfs(&ps("boot_animation_sound"), value),
        ControlId::FanSpeed => write_sysfs(&ps("fan_speed"), value),
        ControlId::LcdOverride => write_sysfs(&ps("lcd_override"), value),
        ControlId::UsbCharging => write_sysfs(&ps("usb_charging"), value),
    }
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

fn read_sysfs(path: &str) -> Result<String> {
    fs::read_to_string(path)
        .map(|content| content.trim().to_string())
        .map_err(|error| sysfs_error(error, "reading", path, None))
}

fn write_sysfs(path: &str, value: &str) -> Result<()> {
    fs::write(path, value).map_err(|error| sysfs_error(error, "writing", path, Some(value)))
}

fn sysfs_error(
    error: std::io::Error,
    action: &str,
    path: &str,
    value: Option<&str>,
) -> anyhow::Error {
    let target = value
        .map(|value| format!(" value '{value}' to {path}"))
        .unwrap_or_else(|| format!(" {path}"));

    if error.kind() == ErrorKind::PermissionDenied {
        anyhow::anyhow!("{action}{target} failed: {error}; {}", setup_hint())
    } else {
        anyhow::anyhow!("{action}{target} failed: {error}")
    }
}

pub(crate) fn apply_rgb_settings(settings: &RgbSettings) -> Result<String> {
    let effect = settings.effect();

    if settings.effect_idx == OFF_EFFECT_INDEX {
        return send_usb_commands(&[PREAMBLE, [0x08, 0x02, 0x01, 0x00, 0x00, 0x01, 0x01, 0x9B]]);
    }

    let mut commands = vec![PREAMBLE];
    if effect.has_color && settings.color_idx != RANDOM_COLOR_INDEX {
        commands.push(make_color_packet(settings.color().rgb));
    }
    commands.push(make_effect_packet(settings));

    send_usb_commands(&commands)
}

pub(crate) fn is_keyboard_present() -> bool {
    keyboard_present()
}

fn make_color_packet(color: Rgb) -> [u8; 8] {
    [0x14, 0x00, 0x00, color.r, color.g, color.b, 0x00, 0x00]
}

fn make_effect_packet(settings: &RgbSettings) -> [u8; 8] {
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

fn send_usb_commands(commands: &[[u8; 8]]) -> Result<String> {
    let handle = open_keyboard()?;
    let was_attached = handle.kernel_driver_active(KB_IFACE).unwrap_or(false);

    if was_attached {
        handle.detach_kernel_driver(KB_IFACE).with_context(|| {
            format!(
                "failed to detach keyboard kernel driver on interface {KB_IFACE}; {}",
                setup_hint()
            )
        })?;
    }

    if let Err(error) = handle
        .claim_interface(KB_IFACE)
        .with_context(|| format!("failed to claim USB interface {KB_IFACE}; {}", setup_hint()))
    {
        if was_attached {
            let _ = handle.attach_kernel_driver(KB_IFACE);
        }
        return Err(error);
    }

    let _ = handle.clear_halt(KB_EP);

    let transfer = (|| -> Result<()> {
        for command in commands {
            handle
                .write_control(0x21, 0x09, 0x0300, KB_IFACE as u16, command, USB_TIMEOUT)
                .with_context(|| {
                    format!("USB control transfer failed for packet {command:02X?}")
                })?;
        }
        Ok(())
    })();

    let release = handle
        .release_interface(KB_IFACE)
        .context("failed to release USB keyboard interface");

    if was_attached {
        let _ = handle.attach_kernel_driver(KB_IFACE);
    }

    transfer?;
    release?;

    Ok("Keyboard lighting applied".to_string())
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
