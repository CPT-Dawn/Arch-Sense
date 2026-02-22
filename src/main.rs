//! # Arch-Sense — Acer Predator Control Center TUI
//!
//! A modern terminal UI for managing Acer Predator laptop settings on Arch Linux.
//! Reads sensor data (CPU/GPU temps, fan speeds) and controls hardware settings
//! via the linuwu_sense kernel module sysfs interface.
//!
//! Run with: `sudo arch-sense`

use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;

// ═══════════════════════════════════════════════════════════════════════════════
//  File Paths
// ═══════════════════════════════════════════════════════════════════════════════

const PS_BASE: &str = "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense";
const PLATFORM_PROFILE: &str = "/sys/firmware/acpi/platform_profile";
const PROFILE_CHOICES: &str = "/sys/firmware/acpi/platform_profile_choices";
const CPU_TEMP_PATH: &str = "/sys/class/thermal/thermal_zone0/temp";

const TICK: Duration = Duration::from_secs(1);

fn ps(name: &str) -> String {
    format!("{PS_BASE}/{name}")
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Theme — Predator Green
// ═══════════════════════════════════════════════════════════════════════════════

struct Theme;

impl Theme {
    // Greens
    const ACCENT: Color = Color::Rgb(57, 255, 20); // neon green
    const ACCENT2: Color = Color::Rgb(0, 200, 60); // medium green
    const DIM: Color = Color::Rgb(0, 140, 40); // dim green
    const DARK: Color = Color::Rgb(0, 60, 20); // dark green
    const BG_HL: Color = Color::Rgb(10, 40, 15); // highlight bg
    const BG_HEADER: Color = Color::Rgb(5, 20, 8); // header bg

    // Text
    const FG: Color = Color::Rgb(210, 225, 210); // primary text
    const FG_DIM: Color = Color::Rgb(100, 130, 100); // dimmed text

    // Temperatures / alerts
    const COOL: Color = Color::Rgb(57, 255, 20); // cool = green
    const WARM: Color = Color::Rgb(255, 200, 0); // warm = yellow
    const HOT: Color = Color::Rgb(255, 50, 30); // hot = red
    const ERR: Color = Color::Rgb(255, 70, 50); // error text

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
//  System I/O — Reading and writing sysfs files
// ═══════════════════════════════════════════════════════════════════════════════

/// Read a sysfs file, returning `None` if it doesn't exist or can't be read.
fn sysfs_read(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Write a value to a sysfs file. Requires root permissions.
fn sysfs_write(path: &str, val: &str) -> Result<()> {
    fs::write(path, val).map_err(|e| anyhow::anyhow!("{e} — writing '{val}' to {path}"))
}

/// Read CPU temperature from thermal_zone0 (returns °C).
fn cpu_temp() -> Option<f64> {
    sysfs_read(CPU_TEMP_PATH)?
        .parse::<f64>()
        .ok()
        .map(|t| t / 1000.0)
}

/// Read GPU temperature via nvidia-smi (returns °C).
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

/// Read CPU and GPU fan speed percentages from linuwu_sense.
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

/// Read available thermal profile choices.
fn thermal_choices() -> Vec<String> {
    sysfs_read(PROFILE_CHOICES)
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Settings Model
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

struct Setting {
    id: Sid,
    label: &'static str,
    desc: &'static str,
    hint: String,
    raw: String,     // raw value from sysfs
    display: String, // human-readable display
}

/// Build / refresh all settings from live sysfs reads.
fn load_settings(choices: &[String]) -> Vec<Setting> {
    let r = |name: &str| sysfs_read(&ps(name)).unwrap_or_else(|| "N/A".into());
    let on_off = |v: &str| match v {
        "1" => "Enabled".into(),
        "0" => "Disabled".into(),
        o => o.to_string(),
    };

    let tp = sysfs_read(PLATFORM_PROFILE).unwrap_or_else(|| "N/A".into());
    let bl = r("backlight_timeout");
    let bc = r("battery_calibration");
    let btl = r("battery_limiter");
    let ba = r("boot_animation_sound");
    let fs = r("fan_speed");
    let lcd = r("lcd_override");
    let usb = r("usb_charging");

    vec![
        Setting {
            id: Sid::Thermal,
            label: "Thermal Profile",
            desc: "Controls CPU/GPU power and thermal behavior",
            hint: if choices.is_empty() {
                "No profiles detected".into()
            } else {
                format!("Options: {}", choices.join(" | "))
            },
            display: tp.clone(),
            raw: tp,
        },
        Setting {
            id: Sid::Backlight,
            label: "Backlight Timeout",
            desc: "Turns off keyboard RGB after 30 seconds of idle",
            hint: "Toggle: 0 (Off) | 1 (On)".into(),
            display: on_off(&bl),
            raw: bl,
        },
        Setting {
            id: Sid::BatCal,
            label: "Battery Calibration",
            desc: "Calibrates battery for accurate readings — keep AC plugged in!",
            hint: "Toggle: 0 (Stop) | 1 (Start calibration)".into(),
            display: match bc.as_str() {
                "1" => "Running".into(),
                "0" => "Stopped".into(),
                o => o.into(),
            },
            raw: bc,
        },
        Setting {
            id: Sid::BatLim,
            label: "Battery Limiter",
            desc: "Limits charging to 80% to preserve battery health",
            hint: "Toggle: 0 (Off) | 1 (Limit to 80%)".into(),
            display: match btl.as_str() {
                "1" => "80% Limit".into(),
                "0" => "Disabled".into(),
                o => o.into(),
            },
            raw: btl,
        },
        Setting {
            id: Sid::BootAnim,
            label: "Boot Animation",
            desc: "Enables or disables custom boot animation and sound",
            hint: "Toggle: 0 (Off) | 1 (On)".into(),
            display: on_off(&ba),
            raw: ba,
        },
        Setting {
            id: Sid::Fan,
            label: "Fan Speed",
            desc: "CPU and GPU fan speeds (0 = Auto, 1-100 = manual %)",
            hint: "Format: CPU,GPU  e.g. 50,70  |  0,0 for Auto".into(),
            display: if fs == "0,0" || fs == "0" {
                "Auto".into()
            } else {
                fs.clone()
            },
            raw: fs,
        },
        Setting {
            id: Sid::Lcd,
            label: "LCD Override",
            desc: "Reduces LCD latency and minimizes ghosting",
            hint: "Toggle: 0 (Off) | 1 (On)".into(),
            display: on_off(&lcd),
            raw: lcd,
        },
        Setting {
            id: Sid::Usb,
            label: "USB Charging",
            desc: "Powers USB port when laptop is off until battery threshold",
            hint: "Values: 0 (Off) | 10 | 20 | 30 (% threshold)".into(),
            display: match usb.as_str() {
                "0" => "Disabled".into(),
                "10" => "Until 10%".into(),
                "20" => "Until 20%".into(),
                "30" => "Until 30%".into(),
                o => o.into(),
            },
            raw: usb,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Validation & Writing
// ═══════════════════════════════════════════════════════════════════════════════

fn validate(id: &Sid, v: &str, choices: &[String]) -> Result<String, String> {
    let v = v.trim();
    match id {
        Sid::Thermal => {
            if choices.iter().any(|c| c == v) {
                Ok(v.into())
            } else {
                Err(format!("Choose from: {}", choices.join(", ")))
            }
        }
        Sid::Backlight | Sid::BatCal | Sid::BatLim | Sid::BootAnim | Sid::Lcd => match v {
            "0" | "1" => Ok(v.into()),
            _ => Err("Must be 0 or 1".into()),
        },
        Sid::Fan => {
            let p: Vec<&str> = v.split(',').collect();
            if p.len() != 2 {
                return Err("Format: CPU,GPU (e.g. 50,70)".into());
            }
            for x in &p {
                match x.trim().parse::<u32>() {
                    Ok(n) if n <= 100 => {}
                    _ => return Err("Each value must be 0-100".into()),
                }
            }
            Ok(format!("{},{}", p[0].trim(), p[1].trim()))
        }
        Sid::Usb => match v {
            "0" | "10" | "20" | "30" => Ok(v.into()),
            _ => Err("Must be 0, 10, 20, or 30".into()),
        },
    }
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

/// Returns true for settings that can be toggled with a single keypress.
fn is_toggle(id: &Sid) -> bool {
    matches!(
        id,
        Sid::Backlight | Sid::BatCal | Sid::BatLim | Sid::BootAnim | Sid::Lcd
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Application State
// ═══════════════════════════════════════════════════════════════════════════════

struct Sensors {
    cpu_t: Option<f64>,
    gpu_t: Option<f64>,
    cpu_f: Option<u32>,
    gpu_f: Option<u32>,
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Edit,
}

struct App {
    sensors: Sensors,
    settings: Vec<Setting>,
    choices: Vec<String>,
    sel: usize,
    mode: Mode,
    input: String,
    curs: usize,
    status: String,
    err: bool,
    quit: bool,
    module_ok: bool,
}

impl App {
    fn new() -> Self {
        let choices = thermal_choices();
        let (cf, gf) = fan_speeds();
        let module_ok = std::path::Path::new(PS_BASE).exists();

        Self {
            sensors: Sensors {
                cpu_t: cpu_temp(),
                gpu_t: gpu_temp(),
                cpu_f: cf,
                gpu_f: gf,
            },
            settings: load_settings(&choices),
            choices,
            sel: 0,
            mode: Mode::Normal,
            input: String::new(),
            curs: 0,
            status: if module_ok {
                "Ready — run as root for write access".into()
            } else {
                "linuwu_sense module not loaded — settings unavailable".into()
            },
            err: !module_ok,
            quit: false,
            module_ok,
        }
    }

    /// Refresh sensor data and settings from sysfs.
    fn tick(&mut self) {
        self.sensors.cpu_t = cpu_temp();
        self.sensors.gpu_t = gpu_temp();
        let (cf, gf) = fan_speeds();
        self.sensors.cpu_f = cf;
        self.sensors.gpu_f = gf;
        // Only refresh settings in normal mode to avoid clobbering edit state
        if self.mode == Mode::Normal {
            self.settings = load_settings(&self.choices);
        }
    }

    /// Toggle a boolean setting directly.
    fn do_toggle(&mut self) {
        let id = self.settings[self.sel].id.clone();
        let name = self.settings[self.sel].label;
        let raw = self.settings[self.sel].raw.clone();
        let new = if raw == "1" { "0" } else { "1" };

        match write_setting(&id, new) {
            Ok(()) => {
                self.status = format!(
                    "  {name} -> {}",
                    if new == "1" { "Enabled" } else { "Disabled" }
                );
                self.err = false;
                self.settings = load_settings(&self.choices);
            }
            Err(e) => {
                self.status = format!("  {e}");
                self.err = true;
            }
        }
    }

    /// Enter edit mode, or toggle if the setting is boolean.
    fn enter_edit(&mut self) {
        if self.settings.is_empty() {
            return;
        }
        let id = self.settings[self.sel].id.clone();

        if is_toggle(&id) {
            self.do_toggle();
            return;
        }

        // Enter text edit mode
        self.input = self.settings[self.sel].raw.clone();
        self.curs = self.input.len();
        self.mode = Mode::Edit;
        self.status = format!(
            "Editing {} — Enter to confirm, Esc to cancel",
            self.settings[self.sel].label
        );
        self.err = false;
    }

    /// Confirm the edit and write the value.
    fn confirm(&mut self) {
        if self.settings.is_empty() {
            return;
        }
        let id = self.settings[self.sel].id.clone();
        let name = self.settings[self.sel].label;

        match validate(&id, &self.input, &self.choices) {
            Ok(v) => match write_setting(&id, &v) {
                Ok(()) => {
                    self.status = format!("  {name} updated -> {v}");
                    self.err = false;
                    self.settings = load_settings(&self.choices);
                }
                Err(e) => {
                    self.status = format!("  Write failed: {e}");
                    self.err = true;
                }
            },
            Err(e) => {
                self.status = format!("  Invalid: {e}");
                self.err = true;
                return; // stay in edit mode
            }
        }
        self.mode = Mode::Normal;
        self.input.clear();
        self.curs = 0;
    }

    /// Cancel editing, return to normal mode.
    fn cancel(&mut self) {
        self.mode = Mode::Normal;
        self.input.clear();
        self.curs = 0;
        self.status = "Cancelled".into();
        self.err = false;
    }

    /// Handle a key event.
    fn on_key(&mut self, k: KeyEvent) {
        // Ctrl+C always quits
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        match self.mode {
            Mode::Normal => match k.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.sel > 0 {
                        self.sel -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.sel + 1 < self.settings.len() {
                        self.sel += 1;
                    }
                }
                KeyCode::Home | KeyCode::Char('g') => self.sel = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    if !self.settings.is_empty() {
                        self.sel = self.settings.len() - 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => self.enter_edit(),
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.tick();
                    self.status = "  Refreshed".into();
                    self.err = false;
                }
                _ => {}
            },
            Mode::Edit => match k.code {
                KeyCode::Esc => self.cancel(),
                KeyCode::Enter => self.confirm(),
                KeyCode::Backspace => {
                    if self.curs > 0 {
                        self.input.remove(self.curs - 1);
                        self.curs -= 1;
                    }
                }
                KeyCode::Delete => {
                    if self.curs < self.input.len() {
                        self.input.remove(self.curs);
                    }
                }
                KeyCode::Left => {
                    if self.curs > 0 {
                        self.curs -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.curs < self.input.len() {
                        self.curs += 1;
                    }
                }
                KeyCode::Home => self.curs = 0,
                KeyCode::End => self.curs = self.input.len(),
                KeyCode::Char(c) => {
                    self.input.insert(self.curs, c);
                    self.curs += 1;
                }
                _ => {}
            },
        }
    }

    /// Main event loop.
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
    let [header, body, detail, status] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(14),
        Constraint::Length(7),
        Constraint::Length(3),
    ])
    .areas(f.area());

    draw_header(f, header);

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(body);

    draw_sensors(f, left, app);
    draw_controls(f, right, app);
    draw_detail(f, detail, app);
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

// ─── Sensor Bar Helper ──────────────────────────────────────────────────────

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

    let sensor_line = |label: &str, val_str: String, color: Color| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {:<20}", label), Style::new().fg(Theme::FG)),
            Span::styled(val_str, Style::new().fg(color).bold()),
        ])
    };

    // CPU Temp
    let cpu_t = app.sensors.cpu_t.unwrap_or(0.0);
    let cpu_str = app
        .sensors
        .cpu_t
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or_else(|| "N/A".into());
    let cpu_c = app
        .sensors
        .cpu_t
        .map(Theme::temp_color)
        .unwrap_or(Theme::FG_DIM);

    // GPU Temp
    let gpu_t = app.sensors.gpu_t.unwrap_or(0.0);
    let gpu_str = app
        .sensors
        .gpu_t
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or_else(|| "N/A".into());
    let gpu_c = app
        .sensors
        .gpu_t
        .map(Theme::temp_color)
        .unwrap_or(Theme::FG_DIM);

    // CPU Fan
    let cpu_f = app.sensors.cpu_f.unwrap_or(0);
    let cpu_f_str = app
        .sensors
        .cpu_f
        .map(|p| {
            if p == 0 {
                "Auto".into()
            } else {
                format!("{p}%")
            }
        })
        .unwrap_or_else(|| "N/A".into());
    let cpu_fc = app
        .sensors
        .cpu_f
        .map(Theme::fan_color)
        .unwrap_or(Theme::FG_DIM);

    // GPU Fan
    let gpu_f = app.sensors.gpu_f.unwrap_or(0);
    let gpu_f_str = app
        .sensors
        .gpu_f
        .map(|p| {
            if p == 0 {
                "Auto".into()
            } else {
                format!("{p}%")
            }
        })
        .unwrap_or_else(|| "N/A".into());
    let gpu_fc = app
        .sensors
        .gpu_f
        .map(Theme::fan_color)
        .unwrap_or(Theme::FG_DIM);

    let lines = vec![
        sensor_line("CPU Temperature", cpu_str, cpu_c),
        make_bar(cpu_t, 105.0, bar_w),
        Line::default(),
        sensor_line("GPU Temperature", gpu_str, gpu_c),
        make_bar(gpu_t, 105.0, bar_w),
        Line::default(),
        sensor_line("CPU Fan", cpu_f_str, cpu_fc),
        make_bar(cpu_f as f64, 100.0, bar_w),
        Line::default(),
        sensor_line("GPU Fan", gpu_f_str, gpu_fc),
        make_bar(gpu_f as f64, 100.0, bar_w),
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
        let msg = Paragraph::new("No settings available")
            .style(Style::new().fg(Theme::FG_DIM))
            .centered();
        f.render_widget(msg, inner);
        return;
    }

    let name_w: u16 = 22;
    let rows: Vec<Row> = app
        .settings
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let sel = i == app.sel;
            let arrow = if sel { " > " } else { "   " };
            let style = if sel {
                Style::new().fg(Theme::ACCENT).bg(Theme::BG_HL).bold()
            } else {
                Style::new().fg(Theme::FG)
            };
            let val_style = if sel {
                Style::new().fg(Theme::ACCENT2).bg(Theme::BG_HL).bold()
            } else {
                Style::new().fg(Theme::DIM)
            };

            let toggle_hint = if is_toggle(&s.id) && s.raw != "N/A" {
                if sel { " [toggle]" } else { "" }
            } else {
                ""
            };

            Row::new(vec![
                Cell::new(arrow).style(style),
                Cell::new(format!("{:<w$}", s.label, w = name_w as usize)).style(style),
                Cell::new(s.display.clone()).style(val_style),
                Cell::new(toggle_hint).style(Style::new().fg(Theme::FG_DIM)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(name_w + 1),
        Constraint::Min(10),
        Constraint::Length(9),
    ];

    let table = Table::new(rows, widths).column_spacing(0);
    f.render_widget(table, inner);
}

// ─── Detail Panel ───────────────────────────────────────────────────────────

fn draw_detail(f: &mut Frame, area: Rect, app: &App) {
    if app.settings.is_empty() {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Theme::DARK))
            .title(Span::styled(
                " Details ",
                Style::new().fg(Theme::ACCENT).bold(),
            ));
        f.render_widget(Paragraph::new("No settings loaded").block(block), area);
        return;
    }

    let s = &app.settings[app.sel];

    let (title, content, border_color) = match app.mode {
        Mode::Normal => {
            let title = format!(" {} ", s.label);
            let content = vec![
                Line::from(vec![
                    Span::styled("  Current: ", Style::new().fg(Theme::FG_DIM)),
                    Span::styled(s.display.clone(), Style::new().fg(Theme::ACCENT).bold()),
                    Span::styled("  |  Raw: ", Style::new().fg(Theme::FG_DIM)),
                    Span::styled(s.raw.clone(), Style::new().fg(Theme::FG)),
                ]),
                Line::from(Span::styled(
                    format!("  {}", s.hint),
                    Style::new().fg(Theme::FG_DIM),
                )),
                Line::default(),
                Line::from(Span::styled(
                    format!("  {}", s.desc),
                    Style::new().fg(Theme::FG).italic(),
                )),
                Line::default(),
            ];
            (title, content, Theme::DIM)
        }
        Mode::Edit => {
            let title = format!("  Edit: {} ", s.label);
            let pos = app.curs.min(app.input.len());
            let before = &app.input[..pos];
            let after = &app.input[pos..];

            let content = vec![
                Line::from(Span::styled(
                    format!("  {}", s.hint),
                    Style::new().fg(Theme::FG_DIM),
                )),
                Line::default(),
                Line::from(vec![
                    Span::styled("  Value: ", Style::new().fg(Theme::FG_DIM)),
                    Span::styled(before.to_string(), Style::new().fg(Theme::ACCENT)),
                    Span::styled("|", Style::new().fg(Theme::ACCENT).bold()),
                    Span::styled(after.to_string(), Style::new().fg(Theme::ACCENT)),
                ]),
                Line::default(),
                Line::from(Span::styled(
                    "  Enter -> Confirm  |  Esc -> Cancel",
                    Style::new().fg(Theme::DIM),
                )),
            ];
            (title, content, Theme::ACCENT)
        }
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border_color))
        .title(Span::styled(title, Style::new().fg(Theme::ACCENT).bold()));

    f.render_widget(Paragraph::new(content).block(block), area);
}

// ─── Status Bar ─────────────────────────────────────────────────────────────

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let mode_span = match app.mode {
        Mode::Normal => Span::styled(
            " NORMAL ",
            Style::new().fg(Color::Black).bg(Theme::ACCENT).bold(),
        ),
        Mode::Edit => Span::styled(
            " EDIT ",
            Style::new().fg(Color::Black).bg(Theme::WARM).bold(),
        ),
    };

    let status_color = if app.err { Theme::ERR } else { Theme::FG_DIM };

    let module_indicator = if app.module_ok {
        Span::styled(" MODULE OK ", Style::new().fg(Theme::COOL).bold())
    } else {
        Span::styled(" NO MODULE ", Style::new().fg(Theme::ERR).bold())
    };

    let help = match app.mode {
        Mode::Normal => " up/down Navigate | Enter Edit/Toggle | r Refresh | q Quit ",
        Mode::Edit => " Type value | Left/Right Cursor | Enter Confirm | Esc Cancel ",
    };

    let lines = vec![
        Line::from(vec![
            mode_span,
            Span::raw(" "),
            module_indicator,
            Span::raw(" "),
            Span::styled(app.status.clone(), Style::new().fg(status_color)),
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
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}
