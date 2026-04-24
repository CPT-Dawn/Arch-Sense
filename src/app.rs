use std::time::Instant;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::AppConfig;
use crate::constants::{PS_BASE, TICK};
use crate::permissions::{keyboard_access, PermissionReport, UsbAccess};
use crate::rgb_settings::{
    load_settings, send_rgb, write_setting, RgbState, Setting, SettingKind, RGB_PARAM_COUNT,
};
use crate::system::{cpu_temp, fan_speeds, gpu_temp, thermal_choices};
use crate::ui::draw;

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Tab {
    System,
    Rgb,
}

pub(crate) struct Sensors {
    pub(crate) cpu_t: Option<f64>,
    pub(crate) gpu_t: Option<f64>,
    pub(crate) cpu_f: Option<u32>,
    pub(crate) gpu_f: Option<u32>,
}

pub struct App {
    pub(crate) tab: Tab,
    pub(crate) sensors: Sensors,
    pub(crate) settings: Vec<Setting>,
    pub(crate) choices: Vec<String>,
    pub(crate) ctrl_sel: usize,
    pub(crate) rgb: RgbState,
    pub(crate) config: AppConfig,
    pub(crate) status: String,
    pub(crate) err: bool,
    pub(crate) quit: bool,
    pub(crate) module_ok: bool,
    pub(crate) tick_n: u64,
}

impl App {
    pub fn new() -> Self {
        let choices = thermal_choices();
        let (cf, gf) = fan_speeds();
        let module_ok = std::path::Path::new(PS_BASE).exists();
        let config = AppConfig::load();
        let rgb = RgbState::from_config(&config.rgb);
        let permissions = PermissionReport::collect();
        let permission_hint = permissions.startup_hint();

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
            status: if !module_ok {
                "⚠ linuwu_sense module not loaded".into()
            } else if let Some(hint) = permission_hint {
                hint
            } else {
                "Ready — F1: System  F2: Keyboard RGB  Tab: Switch".into()
            },
            err: !module_ok || permissions.has_limited_access(),
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
            self.rgb.kb_access = keyboard_access();
            self.rgb.kb_found = !matches!(self.rgb.kb_access, UsbAccess::NotFound);
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
            _ => {}
        }
    }

    fn apply_rgb(&mut self) {
        match send_rgb(&self.rgb) {
            Ok(msg) => {
                // Auto-save on successful apply
                self.config.rgb = self.rgb.to_config();
                match self.config.save() {
                    Ok(()) => {
                        self.status = format!("  ✓ {msg}");
                        self.err = false;
                    }
                    Err(e) => {
                        self.status = format!("  ✓ {msg}; save failed: {e}");
                        self.err = true;
                    }
                }
            }
            Err(e) => {
                self.status = format!("  ✗ RGB: {e}");
                self.err = true;
            }
        }
    }

    // ─── Main Loop ──────────────────────────────────────────────────────────

    pub fn run(mut self, mut term: ratatui::DefaultTerminal) -> Result<()> {
        let mut last = Instant::now();
        loop {
            term.draw(|f| draw(f, &self))?;

            let timeout = TICK.saturating_sub(last.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press {
                        self.on_key(k);
                    }
                }
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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
