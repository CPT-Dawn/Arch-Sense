//! # Arch-Sense — Acer Predator Control Center TUI
//!
//! A modern terminal UI for managing Acer Predator laptop settings on Arch Linux.
//! Controls hardware settings via the linuwu_sense kernel module and
//! keyboard RGB lighting via USB HID protocol (ported from ph16-71-rgb Python).
//!
//! ## Usage
//!   sudo arch-sense          # Launch TUI
//!   sudo arch-sense --apply  # Apply saved RGB settings and exit (for systemd)
//!
//! ## Dependencies
//!   pacman -S libusb         # Required for USB keyboard communication

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Constants & Paths
// ═══════════════════════════════════════════════════════════════════════════════

const PS_BASE: &str = "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense";
const PLATFORM_PROFILE: &str = "/sys/firmware/acpi/platform_profile";
const PROFILE_CHOICES: &str = "/sys/firmware/acpi/platform_profile_choices";
const CPU_TEMP_PATH: &str = "/sys/class/thermal/thermal_zone0/temp";
const TICK: Duration = Duration::from_secs(1);

// USB keyboard (Acer Predator PH16-71)
const KB_VID: u16 = 0x04F2;
const KB_PID: u16 = 0x0117;
const KB_IFACE: u8 = 3;
const KB_EP: u8 = 0x04;
const USB_TIMEOUT: Duration = Duration::from_millis(1000);

// RGB protocol limits
const BRIGHT_HW_MAX: u8 = 50; // 0x32
const SPEED_HW_FAST: u8 = 1;
const SPEED_HW_SLOW: u8 = 9;
const PREAMBLE: [u8; 8] = [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E];

fn ps(name: &str) -> String {
    format!("{PS_BASE}/{name}")
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Theme — Predator Green
// ═══════════════════════════════════════════════════════════════════════════════

struct Theme;

impl Theme {
    const ACCENT: Color = Color::Rgb(57, 255, 20);
    const ACCENT2: Color = Color::Rgb(0, 200, 60);
    const DIM: Color = Color::Rgb(0, 140, 40);
    const DARK: Color = Color::Rgb(0, 60, 20);
    const BG_HL: Color = Color::Rgb(10, 40, 15);
    const BG_HEADER: Color = Color::Rgb(5, 20, 8);
    const FG: Color = Color::Rgb(210, 225, 210);
    const FG_DIM: Color = Color::Rgb(100, 130, 100);
    const COOL: Color = Color::Rgb(57, 255, 20);
    const WARM: Color = Color::Rgb(255, 200, 0);
    const HOT: Color = Color::Rgb(255, 50, 30);
    const ERR: Color = Color::Rgb(255, 70, 50);

    fn temp_color(c: f64) -> Color {
        if c < 55.0 {
            Self::COOL
        } else if c < 78.0 {
            Self::WARM
        } else {
            Self::HOT
        }
    }

    fn fan_color(p: u32) -> Color {
        if p == 0 {
            Self::FG_DIM
        } else if p < 50 {
            Self::COOL
        } else if p < 80 {
            Self::WARM
        } else {
            Self::HOT
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Config Persistence  (~/.config/arch-sense/config.json)
// ═══════════════════════════════════════════════════════════════════════════════

fn config_dir() -> PathBuf {
    // When running via sudo, save config in the real user's home
    let home = std::env::var("SUDO_USER")
        .ok()
        .map(|u| format!("/home/{u}"))
        .or_else(|| std::env::var("HOME").ok())
        .unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".config").join("arch-sense")
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

#[derive(Serialize, Deserialize, Clone)]
struct RgbConfig {
    effect: usize,
    color: usize,
    brightness: u8,
    speed: u8,
    direction: usize,
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
struct AppConfig {
    rgb: RgbConfig,
}

impl AppConfig {
    fn load() -> Self {
        fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<()> {
        fs::create_dir_all(config_dir())?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(config_path(), json)?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  System I/O — Reading & Writing sysfs
// ═══════════════════════════════════════════════════════════════════════════════

fn sysfs_read(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn sysfs_write(path: &str, val: &str) -> Result<()> {
    fs::write(path, val).map_err(|e| anyhow::anyhow!("{e} — writing '{val}' to {path}"))
}

fn cpu_temp() -> Option<f64> {
    sysfs_read(CPU_TEMP_PATH)?
        .parse::<f64>()
        .ok()
        .map(|t| t / 1000.0)
}

fn gpu_temp() -> Option<f64> {
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

fn fan_speeds() -> (Option<u32>, Option<u32>) {
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

fn thermal_choices() -> Vec<String> {
    sysfs_read(PROFILE_CHOICES)
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════════════════════
//  RGB Keyboard USB Protocol (ported from ph16-71-rgb Python)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

const COLOR_PALETTE: &[(&str, Rgb)] = &[
    ("Red", Rgb { r: 255, g: 0, b: 0 }),
    (
        "Orange",
        Rgb {
            r: 255,
            g: 128,
            b: 0,
        },
    ),
    (
        "Gold",
        Rgb {
            r: 255,
            g: 215,
            b: 0,
        },
    ),
    ("Green", Rgb { r: 0, g: 255, b: 0 }),
    (
        "Cyan",
        Rgb {
            r: 0,
            g: 255,
            b: 255,
        },
    ),
    ("Blue", Rgb { r: 0, g: 0, b: 255 }),
    (
        "Purple",
        Rgb {
            r: 128,
            g: 0,
            b: 255,
        },
    ),
    (
        "Magenta",
        Rgb {
            r: 255,
            g: 0,
            b: 255,
        },
    ),
    (
        "Pink",
        Rgb {
            r: 255,
            g: 105,
            b: 180,
        },
    ),
    (
        "White",
        Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
    ),
    ("Random", Rgb { r: 0, g: 0, b: 0 }),
];

const RANDOM_COLOR_IDX: usize = 10;

struct EffectDef {
    name: &'static str,
    opcode: u8, // byte 3 of the effect command
    has_color: bool,
    has_dir: bool,
}

const EFFECTS: &[EffectDef] = &[
    EffectDef {
        name: "Off",
        opcode: 0x01,
        has_color: false,
        has_dir: false,
    },
    EffectDef {
        name: "Static",
        opcode: 0x01,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Breathing",
        opcode: 0x02,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Wave",
        opcode: 0x03,
        has_color: false,
        has_dir: true,
    },
    EffectDef {
        name: "Snake",
        opcode: 0x05,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Ripple",
        opcode: 0x06,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Neon",
        opcode: 0x08,
        has_color: false,
        has_dir: false,
    },
    EffectDef {
        name: "Rain",
        opcode: 0x0A,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Lightning",
        opcode: 0x12,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Spot",
        opcode: 0x25,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Stars",
        opcode: 0x26,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Fireball",
        opcode: 0x27,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Snow",
        opcode: 0x28,
        has_color: true,
        has_dir: false,
    },
    EffectDef {
        name: "Heartbeat",
        opcode: 0x29,
        has_color: true,
        has_dir: false,
    },
];

const OFF_EFFECT_IDX: usize = 0;

const DIRECTIONS: &[&str] = &["Right", "Left", "Up", "Down", "Clockwise", "Counter-CW"];

/// Build the 8-byte color-load packet: 14 00 00 RR GG BB 00 00
fn make_color_pkt(c: Rgb) -> [u8; 8] {
    [0x14, 0x00, 0x00, c.r, c.g, c.b, 0x00, 0x00]
}

/// Build the 8-byte effect packet: 08 02 OP SPEED BRIGHT COLOR_PRESET DIR 9B
fn make_effect_pkt(
    eff: &EffectDef,
    speed_pct: u8,
    bright_pct: u8,
    color_idx: usize,
    dir_idx: usize,
) -> [u8; 8] {
    let hw_bright = ((bright_pct as u16) * BRIGHT_HW_MAX as u16 / 100) as u8;
    let hw_speed = if speed_pct >= 100 {
        SPEED_HW_FAST
    } else {
        let range = (SPEED_HW_SLOW - SPEED_HW_FAST) as u16;
        (SPEED_HW_SLOW - (speed_pct as u16 * range / 100) as u8).max(SPEED_HW_FAST)
    };
    let color_preset: u8 = if color_idx == RANDOM_COLOR_IDX {
        0x08
    } else {
        0x01
    };
    let dir_byte: u8 = if eff.has_dir {
        (dir_idx as u8) + 1
    } else {
        0x01
    };
    [
        0x08,
        0x02,
        eff.opcode,
        hw_speed,
        hw_bright,
        color_preset,
        dir_byte,
        0x9B,
    ]
}

/// Send USB HID commands to the keyboard.
fn send_usb_commands(commands: &[&[u8]]) -> Result<String> {
    let handle = rusb::open_device_with_vid_pid(KB_VID, KB_PID)
        .context("Keyboard not found (VID:04F2 PID:0117). Ensure connected & run with sudo.")?;

    let was_attached = handle.kernel_driver_active(KB_IFACE).unwrap_or(false);
    if was_attached {
        handle
            .detach_kernel_driver(KB_IFACE)
            .context("Failed to detach kernel driver from interface 3")?;
    }

    handle
        .claim_interface(KB_IFACE)
        .context("Failed to claim USB interface 3")?;

    let _ = handle.clear_halt(KB_EP); // ignore errors, not all devices need it

    for cmd in commands {
        // bmRequestType 0x21 = Host-to-Device | Class | Interface
        // bRequest 0x09 = SET_REPORT
        // wValue 0x0300, wIndex = interface 3
        handle
            .write_control(0x21, 0x09, 0x0300, KB_IFACE as u16, cmd, USB_TIMEOUT)
            .context("USB control transfer failed")?;
    }

    handle
        .release_interface(KB_IFACE)
        .context("Failed to release USB interface")?;

    if was_attached {
        let _ = handle.attach_kernel_driver(KB_IFACE);
    }

    Ok("RGB applied successfully".into())
}

/// Apply current RGB state to the keyboard hardware.
fn send_rgb(rgb: &RgbState) -> Result<String> {
    let eff = &EFFECTS[rgb.effect_idx];

    // "Off" = static with brightness 0
    if rgb.effect_idx == OFF_EFFECT_IDX {
        return send_usb_commands(&[&PREAMBLE, &[0x08, 0x02, 0x01, 0x00, 0x00, 0x01, 0x01, 0x9B]]);
    }

    let color_pkt = make_color_pkt(COLOR_PALETTE[rgb.color_idx].1);
    let effect_pkt = make_effect_pkt(eff, rgb.speed, rgb.brightness, rgb.color_idx, rgb.dir_idx);

    let mut cmds: Vec<&[u8]> = vec![&PREAMBLE];
    if eff.has_color && rgb.color_idx != RANDOM_COLOR_IDX {
        cmds.push(&color_pkt);
    }
    cmds.push(&effect_pkt);

    send_usb_commands(&cmds)
}

fn is_kb_present() -> bool {
    rusb::open_device_with_vid_pid(KB_VID, KB_PID).is_some()
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Settings Model (Hardware Controls)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, PartialEq)]
enum Sid {
    Thermal,
    Backlight,
    BatCal,
    BatLim,
    BootAnim,
    Fan,
    Lcd,
    Usb,
}

#[derive(Clone)]
struct CtrlOpt {
    value: String,
    label: String,
}

fn co(v: &str, l: &str) -> CtrlOpt {
    CtrlOpt {
        value: v.into(),
        label: l.into(),
    }
}

#[derive(Clone)]
enum SettingKind {
    Toggle,
    Cycle(Vec<CtrlOpt>),
}

struct Setting {
    id: Sid,
    label: &'static str,
    desc: &'static str,
    raw: String,
    display: String,
    kind: SettingKind,
    pending: Option<usize>, // cycle preview index
}

fn load_settings(choices: &[String]) -> Vec<Setting> {
    let r = |name: &str| sysfs_read(&ps(name)).unwrap_or("N/A".into());
    let on_off = |v: &str| match v {
        "1" => "Enabled".into(),
        "0" => "Disabled".into(),
        o => o.to_string(),
    };

    let tp = sysfs_read(PLATFORM_PROFILE).unwrap_or("N/A".into());
    let bl = r("backlight_timeout");
    let bc = r("battery_calibration");
    let btl = r("battery_limiter");
    let ba = r("boot_animation_sound");
    let fan = r("fan_speed");
    let lcd = r("lcd_override");
    let usb = r("usb_charging");

    let thermal_opts: Vec<CtrlOpt> = if choices.is_empty() {
        vec![co("N/A", "No profiles")]
    } else {
        choices
            .iter()
            .map(|c| {
                let l = match c.as_str() {
                    "quiet" => "Quiet",
                    "balanced" => "Balanced",
                    "performance" => "Performance",
                    "low-power" => "Low-Power",
                    other => other,
                };
                co(c, l)
            })
            .collect()
    };

    vec![
        Setting {
            id: Sid::Thermal,
            label: "Thermal Profile",
            desc: "Controls CPU/GPU power and thermal behavior",
            display: match tp.as_str() {
                "quiet" => "Quiet".into(),
                "balanced" => "Balanced".into(),
                "performance" => "Performance".into(),
                o => o.into(),
            },
            kind: SettingKind::Cycle(thermal_opts),
            pending: None,
            raw: tp,
        },
        Setting {
            id: Sid::Backlight,
            label: "Backlight Timeout",
            desc: "Turns off keyboard RGB after 30s idle",
            display: on_off(&bl),
            kind: SettingKind::Toggle,
            pending: None,
            raw: bl,
        },
        Setting {
            id: Sid::BatCal,
            label: "Battery Calibration",
            desc: "Calibrate battery — keep AC plugged in during calibration!",
            display: match bc.as_str() {
                "1" => "Running".into(),
                "0" => "Stopped".into(),
                o => o.into(),
            },
            kind: SettingKind::Toggle,
            pending: None,
            raw: bc,
        },
        Setting {
            id: Sid::BatLim,
            label: "Battery Limiter",
            desc: "Limits charging to 80% for battery longevity",
            display: match btl.as_str() {
                "1" => "80% Limit".into(),
                "0" => "Disabled".into(),
                o => o.into(),
            },
            kind: SettingKind::Toggle,
            pending: None,
            raw: btl,
        },
        Setting {
            id: Sid::BootAnim,
            label: "Boot Animation",
            desc: "Custom boot animation and sound on startup",
            display: on_off(&ba),
            kind: SettingKind::Toggle,
            pending: None,
            raw: ba,
        },
        Setting {
            id: Sid::Fan,
            label: "Fan Speed",
            desc: "CPU and GPU fan speed control",
            display: if fan == "0,0" || fan == "0" {
                "Auto".into()
            } else {
                format!("CPU/GPU: {fan}")
            },
            kind: SettingKind::Cycle(vec![
                co("0,0", "Auto"),
                co("30,30", "Low (30%)"),
                co("50,50", "Medium (50%)"),
                co("70,70", "High (70%)"),
                co("100,100", "Max (100%)"),
            ]),
            pending: None,
            raw: fan,
        },
        Setting {
            id: Sid::Lcd,
            label: "LCD Override",
            desc: "Reduces LCD latency and minimizes ghosting",
            display: on_off(&lcd),
            kind: SettingKind::Toggle,
            pending: None,
            raw: lcd,
        },
        Setting {
            id: Sid::Usb,
            label: "USB Charging",
            desc: "Powers USB port when laptop is off until battery threshold",
            display: match usb.as_str() {
                "0" => "Disabled".into(),
                "10" => "Until 10%".into(),
                "20" => "Until 20%".into(),
                "30" => "Until 30%".into(),
                o => o.into(),
            },
            kind: SettingKind::Cycle(vec![
                co("0", "Off"),
                co("10", "Until 10%"),
                co("20", "Until 20%"),
                co("30", "Until 30%"),
            ]),
            pending: None,
            raw: usb,
        },
    ]
}

fn write_setting(id: &Sid, v: &str) -> Result<()> {
    match id {
        Sid::Thermal => sysfs_write(PLATFORM_PROFILE, v),
        Sid::Backlight => sysfs_write(&ps("backlight_timeout"), v),
        Sid::BatCal => sysfs_write(&ps("battery_calibration"), v),
        Sid::BatLim => sysfs_write(&ps("battery_limiter"), v),
        Sid::BootAnim => sysfs_write(&ps("boot_animation_sound"), v),
        Sid::Fan => sysfs_write(&ps("fan_speed"), v),
        Sid::Lcd => sysfs_write(&ps("lcd_override"), v),
        Sid::Usb => sysfs_write(&ps("usb_charging"), v),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  RGB State
// ═══════════════════════════════════════════════════════════════════════════════

const RGB_PARAM_COUNT: usize = 5; // effect, color, brightness, speed, direction

struct RgbState {
    effect_idx: usize,
    color_idx: usize,
    brightness: u8, // 0-100 %
    speed: u8,      // 0-100 % (100 = fastest)
    dir_idx: usize,
    sel: usize, // selected parameter row (0..4)
    kb_found: bool,
}

impl RgbState {
    fn from_config(cfg: &RgbConfig) -> Self {
        Self {
            effect_idx: cfg.effect.min(EFFECTS.len() - 1),
            color_idx: cfg.color.min(COLOR_PALETTE.len() - 1),
            brightness: cfg.brightness.min(100),
            speed: cfg.speed.min(100),
            dir_idx: cfg.direction.min(DIRECTIONS.len() - 1),
            sel: 0,
            kb_found: is_kb_present(),
        }
    }

    fn to_config(&self) -> RgbConfig {
        RgbConfig {
            effect: self.effect_idx,
            color: self.color_idx,
            brightness: self.brightness,
            speed: self.speed,
            direction: self.dir_idx,
        }
    }

    fn eff(&self) -> &'static EffectDef {
        &EFFECTS[self.effect_idx]
    }

    fn color_name(&self) -> &'static str {
        COLOR_PALETTE[self.color_idx].0
    }

    fn color_rgb(&self) -> Rgb {
        COLOR_PALETTE[self.color_idx].1
    }

    fn dir_name(&self) -> &'static str {
        DIRECTIONS[self.dir_idx]
    }

    fn cycle_left(&mut self) {
        match self.sel {
            0 => {
                self.effect_idx = if self.effect_idx > 0 {
                    self.effect_idx - 1
                } else {
                    EFFECTS.len() - 1
                }
            }
            1 => {
                self.color_idx = if self.color_idx > 0 {
                    self.color_idx - 1
                } else {
                    COLOR_PALETTE.len() - 1
                }
            }
            2 => self.brightness = self.brightness.saturating_sub(10),
            3 => self.speed = self.speed.saturating_sub(10),
            4 => {
                self.dir_idx = if self.dir_idx > 0 {
                    self.dir_idx - 1
                } else {
                    DIRECTIONS.len() - 1
                }
            }
            _ => {}
        }
    }

    fn cycle_right(&mut self) {
        match self.sel {
            0 => self.effect_idx = (self.effect_idx + 1) % EFFECTS.len(),
            1 => self.color_idx = (self.color_idx + 1) % COLOR_PALETTE.len(),
            2 => self.brightness = (self.brightness + 10).min(100),
            3 => self.speed = (self.speed + 10).min(100),
            4 => self.dir_idx = (self.dir_idx + 1) % DIRECTIONS.len(),
            _ => {}
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Application State
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    System,
    Rgb,
}

struct Sensors {
    cpu_t: Option<f64>,
    gpu_t: Option<f64>,
    cpu_f: Option<u32>,
    gpu_f: Option<u32>,
}

struct App {
    tab: Tab,
    sensors: Sensors,
    settings: Vec<Setting>,
    choices: Vec<String>,
    ctrl_sel: usize,
    rgb: RgbState,
    config: AppConfig,
    status: String,
    err: bool,
    quit: bool,
    module_ok: bool,
    tick_n: u64,
}

impl App {
    fn new() -> Self {
        let choices = thermal_choices();
        let (cf, gf) = fan_speeds();
        let module_ok = std::path::Path::new(PS_BASE).exists();
        let config = AppConfig::load();
        let rgb = RgbState::from_config(&config.rgb);

        Self {
            tab: Tab::System,
            sensors: Sensors {
                cpu_t: cpu_temp(),
                gpu_t: gpu_temp(),
                cpu_f: cf,
                gpu_f: gf,
            },
            settings: load_settings(&choices),
            choices,
            ctrl_sel: 0,
            rgb,
            config,
            status: if module_ok {
                "Ready — F1: System  F2: Keyboard RGB  Tab: Switch".into()
            } else {
                "⚠ linuwu_sense module not loaded".into()
            },
            err: !module_ok,
            quit: false,
            module_ok,
            tick_n: 0,
        }
    }

    fn tick(&mut self) {
        self.sensors.cpu_t = cpu_temp();
        self.sensors.gpu_t = gpu_temp();
        let (cf, gf) = fan_speeds();
        self.sensors.cpu_f = cf;
        self.sensors.gpu_f = gf;
        self.tick_n += 1;

        // Re-check keyboard presence every 5 seconds
        if self.tick_n.is_multiple_of(5) {
            self.rgb.kb_found = is_kb_present();
        }

        // Refresh settings only when no pending cycle preview
        if self.tab == Tab::System && !self.settings.iter().any(|s| s.pending.is_some()) {
            self.settings = load_settings(&self.choices);
        }
    }

    // ─── Key Handling ───────────────────────────────────────────────────────

    fn on_key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        match k.code {
            KeyCode::F(1) => {
                self.tab = Tab::System;
                return;
            }
            KeyCode::F(2) => {
                self.tab = Tab::Rgb;
                return;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.tab = if self.tab == Tab::System {
                    Tab::Rgb
                } else {
                    Tab::System
                };
                return;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.quit = true;
                return;
            }
            _ => {}
        }

        match self.tab {
            Tab::System => self.on_key_system(k),
            Tab::Rgb => self.on_key_rgb(k),
        }
    }

    fn on_key_system(&mut self, k: KeyEvent) {
        let len = self.settings.len();
        if len == 0 {
            return;
        }

        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                // Clear pending on navigation
                self.settings[self.ctrl_sel].pending = None;
                self.ctrl_sel = if self.ctrl_sel > 0 {
                    self.ctrl_sel - 1
                } else {
                    len - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings[self.ctrl_sel].pending = None;
                self.ctrl_sel = (self.ctrl_sel + 1) % len;
            }
            KeyCode::Left | KeyCode::Char('h') => self.cycle_setting_left(),
            KeyCode::Right | KeyCode::Char('l') => self.cycle_setting_right(),
            KeyCode::Enter | KeyCode::Char(' ') => self.confirm_setting(),
            KeyCode::Esc => {
                self.settings[self.ctrl_sel].pending = None;
                self.status = "Cancelled".into();
                self.err = false;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.settings = load_settings(&self.choices);
                self.tick();
                self.status = "  ✓ Refreshed".into();
                self.err = false;
            }
            _ => {}
        }
    }

    fn cycle_setting_left(&mut self) {
        let idx = self.ctrl_sel;
        let raw = self.settings[idx].raw.clone();
        let info = if let SettingKind::Cycle(ref opts) = self.settings[idx].kind {
            if opts.is_empty() {
                return;
            }
            let cur = self.settings[idx]
                .pending
                .unwrap_or_else(|| opts.iter().position(|o| o.value == raw).unwrap_or(0));
            let nxt = if cur > 0 { cur - 1 } else { opts.len() - 1 };
            Some((nxt, opts[nxt].label.clone()))
        } else {
            None
        };
        if let Some((nxt, label)) = info {
            self.settings[idx].pending = Some(nxt);
            self.status = format!("  ◀ {label} — Enter to confirm");
            self.err = false;
        }
    }

    fn cycle_setting_right(&mut self) {
        let idx = self.ctrl_sel;
        let raw = self.settings[idx].raw.clone();
        let info = if let SettingKind::Cycle(ref opts) = self.settings[idx].kind {
            if opts.is_empty() {
                return;
            }
            let cur = self.settings[idx]
                .pending
                .unwrap_or_else(|| opts.iter().position(|o| o.value == raw).unwrap_or(0));
            let nxt = (cur + 1) % opts.len();
            Some((nxt, opts[nxt].label.clone()))
        } else {
            None
        };
        if let Some((nxt, label)) = info {
            self.settings[idx].pending = Some(nxt);
            self.status = format!("  ▶ {label} — Enter to confirm");
            self.err = false;
        }
    }

    fn confirm_setting(&mut self) {
        let idx = self.ctrl_sel;
        let id = self.settings[idx].id.clone();
        let name = self.settings[idx].label;
        let raw = self.settings[idx].raw.clone();
        let is_toggle = matches!(self.settings[idx].kind, SettingKind::Toggle);

        if is_toggle {
            let new_val = if raw == "1" { "0" } else { "1" };
            match write_setting(&id, new_val) {
                Ok(()) => {
                    self.status = format!(
                        "  ✓ {name} → {}",
                        if new_val == "1" {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    );
                    self.err = false;
                    self.settings = load_settings(&self.choices);
                }
                Err(e) => {
                    self.status = format!("  ✗ {e}");
                    self.err = true;
                }
            }
            return;
        }

        // Cycle setting
        let pending = self.settings[idx].pending;
        if let Some(pidx) = pending {
            let write_info = if let SettingKind::Cycle(ref opts) = self.settings[idx].kind {
                opts.get(pidx).map(|o| (o.value.clone(), o.label.clone()))
            } else {
                None
            };
            if let Some((val, label)) = write_info {
                match write_setting(&id, &val) {
                    Ok(()) => {
                        self.status = format!("  ✓ {name} → {label}");
                        self.err = false;
                        self.settings = load_settings(&self.choices);
                    }
                    Err(e) => {
                        self.status = format!("  ✗ {e}");
                        self.err = true;
                    }
                }
            }
        } else {
            // No pending yet: advance to next option as preview
            let info = if let SettingKind::Cycle(ref opts) = self.settings[idx].kind {
                if opts.is_empty() {
                    return;
                }
                let cur = opts.iter().position(|o| o.value == raw).unwrap_or(0);
                let nxt = (cur + 1) % opts.len();
                Some((nxt, opts[nxt].label.clone()))
            } else {
                None
            };
            if let Some((nxt, label)) = info {
                self.settings[idx].pending = Some(nxt);
                self.status = format!("  ▶ {label} — Enter again to confirm, ←→ to browse");
                self.err = false;
            }
        }
    }

    // ─── RGB Key Handling ───────────────────────────────────────────────────

    fn on_key_rgb(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.rgb.sel = if self.rgb.sel > 0 {
                    self.rgb.sel - 1
                } else {
                    RGB_PARAM_COUNT - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.rgb.sel = (self.rgb.sel + 1) % RGB_PARAM_COUNT;
            }
            KeyCode::Left | KeyCode::Char('h') => self.rgb.cycle_left(),
            KeyCode::Right | KeyCode::Char('l') => self.rgb.cycle_right(),
            KeyCode::Enter | KeyCode::Char(' ') => self.apply_rgb(),
            KeyCode::Char('s') | KeyCode::Char('S') => self.save_rgb(),
            _ => {}
        }
    }

    fn apply_rgb(&mut self) {
        match send_rgb(&self.rgb) {
            Ok(msg) => {
                self.status = format!("  ✓ {msg}");
                self.err = false;
                // Auto-save on successful apply
                self.config.rgb = self.rgb.to_config();
                let _ = self.config.save();
            }
            Err(e) => {
                self.status = format!("  ✗ RGB: {e}");
                self.err = true;
            }
        }
    }

    fn save_rgb(&mut self) {
        self.config.rgb = self.rgb.to_config();
        match self.config.save() {
            Ok(()) => {
                self.status = format!("  ✓ Config saved → {}", config_path().display());
                self.err = false;
            }
            Err(e) => {
                self.status = format!("  ✗ Save: {e}");
                self.err = true;
            }
        }
    }

    // ─── Main Loop ──────────────────────────────────────────────────────────

    fn run(mut self, mut term: ratatui::DefaultTerminal) -> Result<()> {
        let mut last = Instant::now();
        loop {
            term.draw(|f| draw(f, &self))?;

            let timeout = TICK.saturating_sub(last.elapsed());
            if event::poll(timeout)?
                && let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
            {
                self.on_key(k);
            }

            if last.elapsed() >= TICK {
                self.tick();
                last = Instant::now();
            }

            if self.quit {
                break;
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  UI Rendering
// ═══════════════════════════════════════════════════════════════════════════════

fn draw(f: &mut Frame, app: &App) {
    let [header, tab_bar, body, detail, status] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(12),
        Constraint::Length(6),
        Constraint::Length(3),
    ])
    .areas(f.area());

    draw_header(f, header);
    draw_tab_bar(f, tab_bar, app);

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(body);

    draw_sensors(f, left, app);

    match app.tab {
        Tab::System => {
            draw_controls(f, right, app);
            draw_detail(f, detail, app);
        }
        Tab::Rgb => {
            draw_rgb_panel(f, right, app);
            draw_rgb_detail(f, detail, app);
        }
    }

    draw_status(f, status, app);
}

// ─── Header ─────────────────────────────────────────────────────────────────

fn draw_header(f: &mut Frame, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(Theme::ACCENT))
        .style(Style::new().bg(Theme::BG_HEADER));

    let text = Line::from(vec![
        Span::styled("  ◆ ", Style::new().fg(Theme::ACCENT).bold()),
        Span::styled("A R C H - S E N S E", Style::new().fg(Theme::ACCENT).bold()),
        Span::styled("  ◆  ", Style::new().fg(Theme::ACCENT)),
        Span::styled(
            "Acer Predator Control Center",
            Style::new().fg(Theme::FG_DIM),
        ),
    ])
    .centered();

    f.render_widget(Paragraph::new(text).block(block), area);
}

// ─── Tab Bar ────────────────────────────────────────────────────────────────

fn draw_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    let sys = if app.tab == Tab::System {
        Style::new().fg(Color::Black).bg(Theme::ACCENT).bold()
    } else {
        Style::new().fg(Theme::FG_DIM)
    };
    let rgb = if app.tab == Tab::Rgb {
        Style::new().fg(Color::Black).bg(Theme::ACCENT).bold()
    } else {
        Style::new().fg(Theme::FG_DIM)
    };

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(" F1 System ", sys),
        Span::raw("  "),
        Span::styled(" F2 Keyboard RGB ", rgb),
        Span::styled(
            "                              Tab to switch",
            Style::new().fg(Theme::DARK),
        ),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

// ─── Sensor Bars ────────────────────────────────────────────────────────────

fn make_bar(val: f64, max: f64, w: u16) -> Line<'static> {
    let ratio = (val / max).clamp(0.0, 1.0);
    let fill = (ratio * w as f64) as usize;
    let empty = (w as usize).saturating_sub(fill);
    let color = if ratio < 0.55 {
        Theme::COOL
    } else if ratio < 0.78 {
        Theme::WARM
    } else {
        Theme::HOT
    };
    Line::from(vec![
        Span::raw("  "),
        Span::styled("━".repeat(fill), Style::new().fg(color)),
        Span::styled("─".repeat(empty), Style::new().fg(Theme::DARK)),
    ])
}

// ─── Sensors Panel ──────────────────────────────────────────────────────────

fn draw_sensors(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " Sensors ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let bar_w = inner.width.saturating_sub(4);

    let sl = |label: &str, val: String, color: Color| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {:<18}", label), Style::new().fg(Theme::FG)),
            Span::styled(val, Style::new().fg(color).bold()),
        ])
    };

    let cpu_t = app.sensors.cpu_t.unwrap_or(0.0);
    let cpu_s = app
        .sensors
        .cpu_t
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or("N/A".into());
    let cpu_c = app
        .sensors
        .cpu_t
        .map(Theme::temp_color)
        .unwrap_or(Theme::FG_DIM);

    let gpu_t = app.sensors.gpu_t.unwrap_or(0.0);
    let gpu_s = app
        .sensors
        .gpu_t
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or("N/A".into());
    let gpu_c = app
        .sensors
        .gpu_t
        .map(Theme::temp_color)
        .unwrap_or(Theme::FG_DIM);

    let cf = app.sensors.cpu_f.unwrap_or(0);
    let cf_s = app
        .sensors
        .cpu_f
        .map(|p| {
            if p == 0 {
                "Auto".into()
            } else {
                format!("{p}%")
            }
        })
        .unwrap_or("N/A".into());
    let cf_c = app
        .sensors
        .cpu_f
        .map(Theme::fan_color)
        .unwrap_or(Theme::FG_DIM);

    let gf = app.sensors.gpu_f.unwrap_or(0);
    let gf_s = app
        .sensors
        .gpu_f
        .map(|p| {
            if p == 0 {
                "Auto".into()
            } else {
                format!("{p}%")
            }
        })
        .unwrap_or("N/A".into());
    let gf_c = app
        .sensors
        .gpu_f
        .map(Theme::fan_color)
        .unwrap_or(Theme::FG_DIM);

    let lines = vec![
        sl("CPU Temperature", cpu_s, cpu_c),
        make_bar(cpu_t, 105.0, bar_w),
        Line::default(),
        sl("GPU Temperature", gpu_s, gpu_c),
        make_bar(gpu_t, 105.0, bar_w),
        Line::default(),
        sl("CPU Fan", cf_s, cf_c),
        make_bar(cf as f64, 100.0, bar_w),
        Line::default(),
        sl("GPU Fan", gf_s, gf_c),
        make_bar(gf as f64, 100.0, bar_w),
    ];

    f.render_widget(Paragraph::new(lines), inner);
}

// ─── Controls Panel ─────────────────────────────────────────────────────────

fn draw_controls(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " Controls ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.settings.is_empty() {
        f.render_widget(
            Paragraph::new("No settings available")
                .style(Style::new().fg(Theme::FG_DIM))
                .centered(),
            inner,
        );
        return;
    }

    let rows: Vec<Row> = app
        .settings
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let sel = i == app.ctrl_sel;
            let arrow = if sel { " ▸ " } else { "   " };
            let style = if sel {
                Style::new().fg(Theme::ACCENT).bg(Theme::BG_HL).bold()
            } else {
                Style::new().fg(Theme::FG)
            };

            // Show pending preview if cycling, else show current
            let disp = if let Some(pidx) = s.pending {
                if let SettingKind::Cycle(ref opts) = s.kind {
                    opts.get(pidx)
                        .map(|o| format!("◀ {} ▶", o.label))
                        .unwrap_or(s.display.clone())
                } else {
                    s.display.clone()
                }
            } else {
                s.display.clone()
            };

            let val_style = if sel && s.pending.is_some() {
                Style::new().fg(Theme::WARM).bg(Theme::BG_HL).bold()
            } else if sel {
                Style::new().fg(Theme::ACCENT2).bg(Theme::BG_HL).bold()
            } else {
                Style::new().fg(Theme::DIM)
            };

            let hint = match (&s.kind, sel) {
                (SettingKind::Toggle, true) => " [Enter]",
                (SettingKind::Cycle(_), true) if s.pending.is_some() => " [Enter]",
                (SettingKind::Cycle(_), true) => " [←→]",
                _ => "",
            };

            Row::new(vec![
                Cell::new(arrow).style(style),
                Cell::new(format!("{:<20}", s.label)).style(style),
                Cell::new(disp).style(val_style),
                Cell::new(hint).style(Style::new().fg(Theme::FG_DIM)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(21),
        Constraint::Min(14),
        Constraint::Length(9),
    ];

    f.render_widget(Table::new(rows, widths).column_spacing(0), inner);
}

// ─── RGB Panel ──────────────────────────────────────────────────────────────

fn draw_rgb_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " Keyboard RGB ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if !app.rgb.kb_found {
        let msg = vec![
            Line::default(),
            Line::from(Span::styled(
                "  ⚠ No compatible keyboard detected",
                Style::new().fg(Theme::WARM),
            )),
            Line::from(Span::styled(
                "    Expected: Acer Predator PH16-71 (04F2:0117)",
                Style::new().fg(Theme::FG_DIM),
            )),
            Line::default(),
            Line::from(Span::styled(
                "    Config can still be edited & saved.",
                Style::new().fg(Theme::DIM),
            )),
            Line::from(Span::styled(
                "    Keyboard will be detected when plugged in.",
                Style::new().fg(Theme::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(msg), inner);
        return;
    }

    let eff = app.rgb.eff();
    let bar_w: usize = 20;

    let mk_row = |idx: usize, label: &str, spans: Vec<Span<'static>>| -> Vec<Line<'static>> {
        let sel = idx == app.rgb.sel;
        let arr = if sel { " ▸ " } else { "   " };
        let ls = if sel {
            Style::new().fg(Theme::ACCENT).bold()
        } else {
            Style::new().fg(Theme::FG)
        };
        let mut all = vec![
            Span::styled(String::from(arr), ls),
            Span::styled(format!("{:<14}", label), ls),
        ];
        all.extend(spans);
        vec![Line::from(all)]
    };

    // Effect
    let effect_spans = vec![
        Span::styled("◀ ", Style::new().fg(Theme::DIM)),
        Span::styled(
            String::from(eff.name),
            Style::new().fg(Theme::ACCENT2).bold(),
        ),
        Span::styled(" ▶", Style::new().fg(Theme::DIM)),
    ];

    // Color
    let c = app.rgb.color_rgb();
    let cn = app.rgb.color_name();
    let color_spans = if eff.has_color {
        let swatch = if app.rgb.color_idx == RANDOM_COLOR_IDX {
            Span::styled(" ◆◆◆ ", Style::new().fg(Theme::ACCENT))
        } else {
            Span::styled(" ███ ", Style::new().fg(Color::Rgb(c.r, c.g, c.b)))
        };
        vec![
            Span::styled("◀ ", Style::new().fg(Theme::DIM)),
            Span::styled(String::from(cn), Style::new().fg(Theme::ACCENT2).bold()),
            Span::styled(" ▶ ", Style::new().fg(Theme::DIM)),
            swatch,
        ]
    } else {
        vec![Span::styled(
            "  N/A (effect has no color)",
            Style::new().fg(Theme::DARK),
        )]
    };

    // Brightness bar
    let bf = (app.rgb.brightness as usize * bar_w / 100).min(bar_w);
    let be = bar_w.saturating_sub(bf);
    let bright_spans = vec![
        Span::styled("━".repeat(bf), Style::new().fg(Theme::ACCENT)),
        Span::styled("─".repeat(be), Style::new().fg(Theme::DARK)),
        Span::styled(
            format!(" {}%", app.rgb.brightness),
            Style::new().fg(Theme::FG).bold(),
        ),
    ];

    // Speed bar
    let sf = (app.rgb.speed as usize * bar_w / 100).min(bar_w);
    let se = bar_w.saturating_sub(sf);
    let speed_spans = vec![
        Span::styled("━".repeat(sf), Style::new().fg(Theme::ACCENT)),
        Span::styled("─".repeat(se), Style::new().fg(Theme::DARK)),
        Span::styled(
            format!(" {}%", app.rgb.speed),
            Style::new().fg(Theme::FG).bold(),
        ),
    ];

    // Direction
    let dir_spans = if eff.has_dir {
        vec![
            Span::styled("◀ ", Style::new().fg(Theme::DIM)),
            Span::styled(
                String::from(app.rgb.dir_name()),
                Style::new().fg(Theme::ACCENT2).bold(),
            ),
            Span::styled(" ▶", Style::new().fg(Theme::DIM)),
        ]
    } else {
        vec![Span::styled(
            "  N/A (Wave only)",
            Style::new().fg(Theme::DARK),
        )]
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.extend(mk_row(0, "Effect", effect_spans));
    lines.push(Line::default());
    lines.extend(mk_row(1, "Color", color_spans));
    lines.push(Line::default());
    lines.extend(mk_row(2, "Brightness", bright_spans));
    lines.push(Line::default());
    lines.extend(mk_row(3, "Speed", speed_spans));
    lines.push(Line::default());
    lines.extend(mk_row(4, "Direction", dir_spans));

    f.render_widget(Paragraph::new(lines), inner);
}

// ─── Detail Panel (System Tab) ──────────────────────────────────────────────

fn draw_detail(f: &mut Frame, area: Rect, app: &App) {
    if app.settings.is_empty() {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Theme::DARK))
            .title(Span::styled(
                " Details ",
                Style::new().fg(Theme::ACCENT).bold(),
            ));
        f.render_widget(Paragraph::new("  No settings loaded").block(block), area);
        return;
    }

    let s = &app.settings[app.ctrl_sel];
    let border = if s.pending.is_some() {
        Theme::WARM
    } else {
        Theme::DIM
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(Span::styled(
            format!(" {} ", s.label),
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let mut lines = vec![
        Line::from(vec![
            Span::styled("  Current: ", Style::new().fg(Theme::FG_DIM)),
            Span::styled(s.display.clone(), Style::new().fg(Theme::ACCENT).bold()),
            Span::styled("  │  Raw: ", Style::new().fg(Theme::FG_DIM)),
            Span::styled(s.raw.clone(), Style::new().fg(Theme::FG)),
        ]),
        Line::from(Span::styled(
            format!("  {}", s.desc),
            Style::new().fg(Theme::FG).italic(),
        )),
    ];

    if let Some(pidx) = s.pending
        && let SettingKind::Cycle(ref opts) = s.kind
        && let Some(opt) = opts.get(pidx)
    {
        lines.push(Line::from(vec![
            Span::styled("  Preview: ", Style::new().fg(Theme::WARM)),
            Span::styled(opt.label.clone(), Style::new().fg(Theme::WARM).bold()),
            Span::styled("  → Enter to apply", Style::new().fg(Theme::FG_DIM)),
        ]));
    }

    let hint = match &s.kind {
        SettingKind::Toggle => "  Enter: Toggle  │  ↑↓: Navigate".into(),
        SettingKind::Cycle(opts) => {
            let names: Vec<&str> = opts.iter().map(|o| o.label.as_str()).collect();
            format!("  ←→: [{}]  │  Enter: Confirm", names.join(" │ "))
        }
    };
    lines.push(Line::from(Span::styled(hint, Style::new().fg(Theme::DIM))));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

// ─── Detail Panel (RGB Tab) ─────────────────────────────────────────────────

fn draw_rgb_detail(f: &mut Frame, area: Rect, app: &App) {
    let eff = app.rgb.eff();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " RGB Details ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let desc = match app.rgb.sel {
        0 => format!(
            "  {} — {}/{} effects. ←→ to browse.",
            eff.name,
            app.rgb.effect_idx + 1,
            EFFECTS.len()
        ),
        1 => format!(
            "  {} — {}/{} colors. ←→ to cycle.",
            app.rgb.color_name(),
            app.rgb.color_idx + 1,
            COLOR_PALETTE.len()
        ),
        2 => format!(
            "  Brightness {}% — LED intensity. ←→ adjusts ±10%.",
            app.rgb.brightness
        ),
        3 => format!(
            "  Speed {}% — Animation speed (100 = fastest). ←→ adjusts ±10%.",
            app.rgb.speed
        ),
        4 => format!("  {} — Wave direction. ←→ to cycle.", app.rgb.dir_name()),
        _ => String::new(),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("  Preview: ", Style::new().fg(Theme::FG_DIM)),
            Span::styled(
                String::from(eff.name),
                Style::new().fg(Theme::ACCENT2).bold(),
            ),
            if eff.has_color {
                Span::styled(
                    format!(" │ {} ", app.rgb.color_name()),
                    Style::new().fg(Theme::FG),
                )
            } else {
                Span::raw("")
            },
            Span::styled(
                format!("│ B:{}% S:{}%", app.rgb.brightness, app.rgb.speed),
                Style::new().fg(Theme::FG),
            ),
            if eff.has_dir {
                Span::styled(
                    format!(" │ Dir:{}", app.rgb.dir_name()),
                    Style::new().fg(Theme::FG),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(Span::styled(desc, Style::new().fg(Theme::FG_DIM))),
        Line::default(),
        Line::from(Span::styled(
            "  Enter: Apply to keyboard  │  S: Save config  │  ←→: Adjust  │  ↑↓: Param",
            Style::new().fg(Theme::DIM),
        )),
    ];

    f.render_widget(Paragraph::new(lines).block(block), area);
}

// ─── Status Bar ─────────────────────────────────────────────────────────────

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let tab_span = match app.tab {
        Tab::System => Span::styled(
            " SYSTEM ",
            Style::new().fg(Color::Black).bg(Theme::ACCENT).bold(),
        ),
        Tab::Rgb => Span::styled(
            " RGB ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Rgb(128, 0, 255))
                .bold(),
        ),
    };

    let module_span = if app.module_ok {
        Span::styled(" MODULE ✓ ", Style::new().fg(Theme::COOL).bold())
    } else {
        Span::styled(" NO MODULE ", Style::new().fg(Theme::ERR).bold())
    };

    let kb_span = if app.rgb.kb_found {
        Span::styled(" KB ✓ ", Style::new().fg(Theme::COOL).bold())
    } else {
        Span::styled(" NO KB ", Style::new().fg(Theme::WARM).bold())
    };

    let sc = if app.err { Theme::ERR } else { Theme::FG_DIM };

    let help = match app.tab {
        Tab::System => " F1/F2 Tab │ ↑↓ Navigate │ ←→ Cycle │ Enter Confirm/Toggle │ q Quit ",
        Tab::Rgb => " F1/F2 Tab │ ↑↓ Param │ ←→ Adjust │ Enter Apply │ S Save │ q Quit ",
    };

    let lines = vec![
        Line::from(vec![
            tab_span,
            Span::raw(" "),
            module_span,
            kb_span,
            Span::raw(" "),
            Span::styled(app.status.clone(), Style::new().fg(sc)),
        ]),
        Line::from(Span::styled(help, Style::new().fg(Theme::FG_DIM))),
    ];

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DARK));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Entrypoint
// ═══════════════════════════════════════════════════════════════════════════════

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // --help
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("Arch-Sense — Acer Predator Control Center\n");
        eprintln!("Usage:");
        eprintln!("  sudo arch-sense            Launch TUI");
        eprintln!("  sudo arch-sense --apply    Apply saved RGB settings (for boot/systemd)");
        eprintln!("\nConfig: {}", config_path().display());
        eprintln!("Systemd: sudo cp arch-sense.service /etc/systemd/system/");
        eprintln!("         sudo systemctl enable --now arch-sense");
        return Ok(());
    }

    // --apply: headless mode for systemd / boot
    if args.iter().any(|a| a == "--apply") {
        return apply_saved_config();
    }

    // Normal TUI mode
    let terminal = ratatui::init();
    let app = App::new();

    // Apply saved RGB on startup
    if app.rgb.kb_found {
        let _ = send_rgb(&app.rgb);
    }

    let result = app.run(terminal);
    ratatui::restore();
    result
}

/// Headless: apply saved RGB config and exit (for systemd service / boot).
fn apply_saved_config() -> Result<()> {
    let config = AppConfig::load();
    let rgb = RgbState::from_config(&config.rgb);

    if !is_kb_present() {
        eprintln!("arch-sense: Keyboard not found (VID:04F2 PID:0117)");
        std::process::exit(0);
    }

    match send_rgb(&rgb) {
        Ok(msg) => {
            eprintln!("arch-sense: {msg}");
            Ok(())
        }
        Err(e) => {
            eprintln!("arch-sense: RGB apply failed: {e}");
            Err(e)
        }
    }
}
