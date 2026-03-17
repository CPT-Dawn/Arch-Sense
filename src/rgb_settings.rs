use anyhow::{Context, Result};

use crate::config::RgbConfig;
use crate::constants::{
    BRIGHT_HW_MAX, KB_EP, KB_IFACE, KB_PID, KB_VID, PLATFORM_PROFILE, PREAMBLE, SPEED_HW_FAST,
    SPEED_HW_SLOW, USB_TIMEOUT, ps,
};
use crate::system::{sysfs_read, sysfs_write};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Rgb {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

pub(crate) const COLOR_PALETTE: &[(&str, Rgb)] = &[
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

pub(crate) const RANDOM_COLOR_IDX: usize = 10;

pub(crate) struct EffectDef {
    pub(crate) name: &'static str,
    pub(crate) opcode: u8, // byte 3 of the effect command
    pub(crate) has_color: bool,
    pub(crate) has_dir: bool,
}

pub(crate) const EFFECTS: &[EffectDef] = &[
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
pub(crate) fn send_rgb(rgb: &RgbState) -> Result<String> {
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

pub(crate) fn is_kb_present() -> bool {
    rusb::open_device_with_vid_pid(KB_VID, KB_PID).is_some()
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Settings Model (Hardware Controls)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, PartialEq)]
pub(crate) enum Sid {
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
pub(crate) struct CtrlOpt {
    pub(crate) value: String,
    pub(crate) label: String,
}

fn co(v: &str, l: &str) -> CtrlOpt {
    CtrlOpt {
        value: v.into(),
        label: l.into(),
    }
}

#[derive(Clone)]
pub(crate) enum SettingKind {
    Toggle,
    Cycle(Vec<CtrlOpt>),
}

pub(crate) struct Setting {
    pub(crate) id: Sid,
    pub(crate) label: &'static str,
    pub(crate) desc: &'static str,
    pub(crate) raw: String,
    pub(crate) display: String,
    pub(crate) kind: SettingKind,
    pub(crate) pending: Option<usize>, // cycle preview index
}

pub(crate) fn load_settings(choices: &[String]) -> Vec<Setting> {
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

pub(crate) fn write_setting(id: &Sid, v: &str) -> Result<()> {
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

pub(crate) const RGB_PARAM_COUNT: usize = 5; // effect, color, brightness, speed, direction

pub(crate) struct RgbState {
    pub(crate) effect_idx: usize,
    pub(crate) color_idx: usize,
    pub(crate) brightness: u8, // 0-100 %
    pub(crate) speed: u8,      // 0-100 % (100 = fastest)
    pub(crate) dir_idx: usize,
    pub(crate) sel: usize, // selected parameter row (0..4)
    pub(crate) kb_found: bool,
}

impl RgbState {
    pub(crate) fn from_config(cfg: &RgbConfig) -> Self {
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

    pub(crate) fn to_config(&self) -> RgbConfig {
        RgbConfig {
            effect: self.effect_idx,
            color: self.color_idx,
            brightness: self.brightness,
            speed: self.speed,
            direction: self.dir_idx,
        }
    }

    pub(crate) fn eff(&self) -> &'static EffectDef {
        &EFFECTS[self.effect_idx]
    }

    pub(crate) fn color_name(&self) -> &'static str {
        COLOR_PALETTE[self.color_idx].0
    }

    pub(crate) fn color_rgb(&self) -> Rgb {
        COLOR_PALETTE[self.color_idx].1
    }

    pub(crate) fn dir_name(&self) -> &'static str {
        DIRECTIONS[self.dir_idx]
    }

    pub(crate) fn cycle_left(&mut self) {
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

    pub(crate) fn cycle_right(&mut self) {
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
