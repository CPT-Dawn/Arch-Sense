use std::collections::VecDeque;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::AppConfig;
use crate::hardware::{spawn_worker, HardwareEvent, HardwareHandle, HardwareRequest};
use crate::models::{
    ControlId, ControlItem, ControlKind, FanMode, FocusPanel, RgbField, RgbSettings, SensorMetric,
    SensorSnapshot,
};
use crate::permissions::UsbAccess;
use crate::ui::draw;

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(1);
const HISTORY_LIMIT: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MessageLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub(crate) struct StatusMessage {
    pub(crate) level: MessageLevel,
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AnimatedMetric {
    pub(crate) value: f64,
    pub(crate) target: Option<f64>,
    pub(crate) max: f64,
    pub(crate) error: Option<String>,
}

impl AnimatedMetric {
    fn new(max: f64) -> Self {
        Self {
            value: 0.0,
            target: None,
            max,
            error: None,
        }
    }

    fn update(&mut self, metric: &SensorMetric) {
        self.target = metric.value;
        self.error = metric.error.clone();
    }

    fn advance(&mut self, dt: Duration) {
        let Some(target) = self.target else {
            return;
        };

        let rate = 1.0 - (-10.0 * dt.as_secs_f64()).exp();
        self.value += (target - self.value) * rate;
        if (self.value - target).abs() < 0.05 {
            self.value = target;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SensorsState {
    pub(crate) cpu_temp: AnimatedMetric,
    pub(crate) gpu_temp: AnimatedMetric,
    pub(crate) cpu_fan: AnimatedMetric,
    pub(crate) gpu_fan: AnimatedMetric,
    pub(crate) cpu_temp_history: VecDeque<u64>,
    pub(crate) gpu_temp_history: VecDeque<u64>,
    pub(crate) cpu_fan_history: VecDeque<u64>,
    pub(crate) gpu_fan_history: VecDeque<u64>,
    pub(crate) cpu_fan_mode: FanMode,
    pub(crate) gpu_fan_mode: FanMode,
}

impl SensorsState {
    fn new() -> Self {
        Self {
            cpu_temp: AnimatedMetric::new(105.0),
            gpu_temp: AnimatedMetric::new(105.0),
            cpu_fan: AnimatedMetric::new(7000.0),
            gpu_fan: AnimatedMetric::new(7000.0),
            cpu_temp_history: VecDeque::with_capacity(HISTORY_LIMIT),
            gpu_temp_history: VecDeque::with_capacity(HISTORY_LIMIT),
            cpu_fan_history: VecDeque::with_capacity(HISTORY_LIMIT),
            gpu_fan_history: VecDeque::with_capacity(HISTORY_LIMIT),
            cpu_fan_mode: FanMode::Auto,
            gpu_fan_mode: FanMode::Auto,
        }
    }

    fn update(&mut self, snapshot: &SensorSnapshot) {
        self.cpu_temp.update(&snapshot.cpu_temp);
        self.gpu_temp.update(&snapshot.gpu_temp);
        self.cpu_fan.update(&snapshot.cpu_fan);
        self.gpu_fan.update(&snapshot.gpu_fan);
        Self::push_history(
            &mut self.cpu_temp_history,
            snapshot.cpu_temp.value,
            self.cpu_temp.max,
        );
        Self::push_history(
            &mut self.gpu_temp_history,
            snapshot.gpu_temp.value,
            self.gpu_temp.max,
        );
        Self::push_history(
            &mut self.cpu_fan_history,
            snapshot.cpu_fan.value,
            self.cpu_fan.max,
        );
        Self::push_history(
            &mut self.gpu_fan_history,
            snapshot.gpu_fan.value,
            self.gpu_fan.max,
        );
        self.cpu_fan_mode = snapshot.cpu_fan_mode;
        self.gpu_fan_mode = snapshot.gpu_fan_mode;
    }

    fn advance(&mut self, dt: Duration) {
        self.cpu_temp.advance(dt);
        self.gpu_temp.advance(dt);
        self.cpu_fan.advance(dt);
        self.gpu_fan.advance(dt);
    }

    fn push_history(history: &mut VecDeque<u64>, value: Option<f64>, max: f64) {
        let clamped = value.unwrap_or(0.0).clamp(0.0, max).round() as u64;
        history.push_back(clamped);

        while history.len() > HISTORY_LIMIT {
            let _ = history.pop_front();
        }
    }
}

pub struct App {
    pub(crate) focus: FocusPanel,
    pub(crate) controls: Vec<ControlItem>,
    pub(crate) selected_control: usize,
    pub(crate) rgb: RgbSettings,
    pub(crate) selected_rgb_field: usize,
    pub(crate) sensors: SensorsState,
    pub(crate) module_loaded: bool,
    pub(crate) keyboard: UsbAccess,
    pub(crate) message: StatusMessage,
    pub(crate) hardware_note: Option<String>,
    pub(crate) snapshot_pending: bool,
    pub(crate) control_pending: Option<ControlId>,
    pub(crate) rgb_pending: bool,
    pub(crate) rgb_dirty: bool,
    pub(crate) focus_pulse: f64,
    pub(crate) rgb_phase: f64,
    config: AppConfig,
    hardware: HardwareHandle,
    last_snapshot_request: Instant,
    quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let (config, config_warning) = AppConfig::load_with_warning();
        let rgb = RgbSettings::from_config(&config.rgb);
        let hardware = spawn_worker()?;
        let now = Instant::now();

        let mut app = Self {
            focus: FocusPanel::Controls,
            controls: Vec::new(),
            selected_control: 0,
            rgb,
            selected_rgb_field: 0,
            sensors: SensorsState::new(),
            module_loaded: false,
            keyboard: UsbAccess::NotFound,
            message: StatusMessage {
                level: MessageLevel::Info,
                text: config_warning.unwrap_or_else(|| "Starting hardware scan".to_string()),
            },
            hardware_note: None,
            snapshot_pending: false,
            control_pending: None,
            rgb_pending: false,
            rgb_dirty: false,
            focus_pulse: 1.0,
            rgb_phase: 0.0,
            config,
            hardware,
            last_snapshot_request: now - SNAPSHOT_INTERVAL,
            quit: false,
        };
        app.request_snapshot();
        Ok(app)
    }

    pub fn run(mut self, mut terminal: ratatui::DefaultTerminal) -> Result<()> {
        let mut last_frame = Instant::now();

        loop {
            let frame_started = Instant::now();
            let delta = frame_started.saturating_duration_since(last_frame);
            last_frame = frame_started;

            self.on_frame(delta);
            terminal.draw(|frame| draw(frame, &self))?;

            if self.quit {
                break;
            }

            let timeout = FRAME_INTERVAL.saturating_sub(frame_started.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.on_key(key);
                    }
                }
            }
        }

        Ok(())
    }

    fn on_frame(&mut self, dt: Duration) {
        self.sensors.advance(dt);
        self.focus_pulse = (self.focus_pulse - dt.as_secs_f64() * 3.2).max(0.0);
        self.rgb_phase = (self.rgb_phase + dt.as_secs_f64() * 18.0) % 1000.0;
        self.handle_hardware_events();

        if self.last_snapshot_request.elapsed() >= SNAPSHOT_INTERVAL {
            self.request_snapshot();
        }
    }

    fn request_snapshot(&mut self) {
        if self.snapshot_pending {
            return;
        }

        match self.hardware.send(HardwareRequest::Snapshot) {
            Ok(()) => {
                self.snapshot_pending = true;
                self.last_snapshot_request = Instant::now();
            }
            Err(error) => self.set_message(MessageLevel::Error, error.to_string()),
        }
    }

    fn handle_hardware_events(&mut self) {
        for event in self.hardware.drain() {
            match event {
                HardwareEvent::Snapshot(snapshot) => {
                    let snapshot = *snapshot;
                    self.snapshot_pending = false;
                    self.module_loaded = snapshot.module_loaded;
                    self.keyboard = snapshot.keyboard;
                    self.hardware_note = snapshot.note;
                    self.sensors.update(&snapshot.sensors);
                    self.replace_controls(snapshot.controls, true);

                    if self.message.text == "Starting hardware scan" {
                        self.set_message(MessageLevel::Success, "Hardware scan complete");
                    }
                }
                HardwareEvent::ControlApplied { id, controls } => {
                    self.control_pending = None;
                    self.clear_pending_controls();
                    self.replace_controls(controls, false);
                    self.set_message(MessageLevel::Success, format!("{} applied", id.label()));
                }
                HardwareEvent::ControlFailed { id, error } => {
                    self.control_pending = None;
                    self.set_message(
                        MessageLevel::Error,
                        format!("{} failed: {error}", id.label()),
                    );
                    self.mark_control_error(id, error);
                    self.clear_pending_controls();
                }
                HardwareEvent::RgbApplied(message) => {
                    self.rgb_pending = false;
                    self.rgb_dirty = false;
                    self.config.rgb = self.rgb.to_config();
                    match self.config.save() {
                        Ok(()) => self.set_message(MessageLevel::Success, message),
                        Err(error) => self.set_message(
                            MessageLevel::Error,
                            format!("{message}; config save failed: {error}"),
                        ),
                    }
                }
                HardwareEvent::RgbFailed(error) => {
                    self.rgb_pending = false;
                    self.set_message(MessageLevel::Error, format!("RGB apply failed: {error}"));
                }
            }
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.quit = true;
            }
            KeyCode::Tab => self.set_focus(self.focus.next()),
            KeyCode::BackTab => self.set_focus(self.focus.previous()),
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.request_snapshot();
                self.set_message(MessageLevel::Info, "Refresh requested");
            }
            KeyCode::Esc => {
                self.clear_pending_controls();
                self.set_message(MessageLevel::Info, "Pending change cancelled");
            }
            _ => match self.focus {
                FocusPanel::Controls => self.on_controls_key(key),
                FocusPanel::Rgb => self.on_rgb_key(key),
                FocusPanel::Sensors => self.on_sensors_key(key),
            },
        }
    }

    fn set_focus(&mut self, focus: FocusPanel) {
        if self.focus != focus {
            self.focus = focus;
            self.focus_pulse = 1.0;
        }
    }

    fn on_controls_key(&mut self, key: KeyEvent) {
        if self.controls.is_empty() {
            return;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_control_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_control_selection(1),
            KeyCode::Left | KeyCode::Char('h') => self.cycle_control(-1),
            KeyCode::Right | KeyCode::Char('l') => self.cycle_control(1),
            KeyCode::Enter | KeyCode::Char(' ') => self.apply_selected_control(),
            _ => {}
        }
    }

    fn on_rgb_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_rgb_field = self
                    .selected_rgb_field
                    .checked_sub(1)
                    .unwrap_or(RgbField::ALL.len() - 1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected_rgb_field = (self.selected_rgb_field + 1) % RgbField::ALL.len();
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust_rgb(-1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust_rgb(1),
            KeyCode::Enter | KeyCode::Char(' ') => self.apply_rgb(),
            _ => {}
        }
    }

    fn on_sensors_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            self.request_snapshot();
            self.set_message(MessageLevel::Info, "Sensor refresh requested");
        }
    }

    fn move_control_selection(&mut self, step: isize) {
        self.clear_pending_controls();
        let len = self.controls.len();
        if step < 0 {
            self.selected_control = self.selected_control.checked_sub(1).unwrap_or(len - 1);
        } else {
            self.selected_control = (self.selected_control + 1) % len;
        }
    }

    fn cycle_control(&mut self, step: i8) {
        let Some(message) = ({
            let Some(item) = self.controls.get_mut(self.selected_control) else {
                return;
            };

            match &item.kind {
                ControlKind::Toggle => {
                    Some((MessageLevel::Info, "Enter toggles this setting".to_string()))
                }
                ControlKind::Choice(choices) if choices.is_empty() => Some((
                    MessageLevel::Warning,
                    "No choices are available".to_string(),
                )),
                ControlKind::Choice(choices) => {
                    let current = item
                        .pending
                        .or_else(|| item.current_choice_index())
                        .unwrap_or(0);
                    let next = if step < 0 {
                        current.checked_sub(1).unwrap_or(choices.len() - 1)
                    } else {
                        (current + 1) % choices.len()
                    };
                    item.pending = Some(next);
                    Some((
                        MessageLevel::Info,
                        format!("Preview {}: {}", item.label(), choices[next].label),
                    ))
                }
            }
        }) else {
            return;
        };

        self.set_message(message.0, message.1);
    }

    fn apply_selected_control(&mut self) {
        if self.control_pending.is_some() {
            self.set_message(
                MessageLevel::Warning,
                "A control write is already in progress",
            );
            return;
        }

        let Some(item) = self.controls.get(self.selected_control) else {
            return;
        };

        let request = match &item.kind {
            ControlKind::Toggle => {
                let value = if item.raw == "1" { "0" } else { "1" };
                Some((item.id, value.to_string()))
            }
            ControlKind::Choice(choices) => {
                let Some(index) = item.pending else {
                    self.cycle_control(1);
                    return;
                };
                choices
                    .get(index)
                    .map(|choice| (item.id, choice.value.clone()))
            }
        };

        let Some((id, value)) = request else {
            self.set_message(MessageLevel::Warning, "No valid value selected");
            return;
        };

        match self
            .hardware
            .send(HardwareRequest::ApplyControl { id, value })
        {
            Ok(()) => {
                self.control_pending = Some(id);
                self.set_message(MessageLevel::Info, format!("Applying {}", id.label()));
            }
            Err(error) => self.set_message(MessageLevel::Error, error.to_string()),
        }
    }

    fn adjust_rgb(&mut self, step: i8) {
        let field = RgbField::ALL[self.selected_rgb_field];
        self.rgb.adjust(field, step);
        self.rgb_dirty = true;
        self.focus_pulse = 1.0;
        self.set_message(
            MessageLevel::Info,
            format!("{} changed; Enter applies lighting", field.label()),
        );
    }

    fn apply_rgb(&mut self) {
        if self.rgb_pending {
            self.set_message(MessageLevel::Warning, "RGB write is already in progress");
            return;
        }

        match self
            .hardware
            .send(HardwareRequest::ApplyRgb(self.rgb))
        {
            Ok(()) => {
                self.rgb_pending = true;
                self.set_message(MessageLevel::Info, "Applying keyboard lighting");
            }
            Err(error) => self.set_message(MessageLevel::Error, error.to_string()),
        }
    }

    fn replace_controls(&mut self, mut controls: Vec<ControlItem>, preserve_pending: bool) {
        let selected_id = self.controls.get(self.selected_control).map(|item| item.id);

        if preserve_pending {
            for incoming in &mut controls {
                if let Some(existing) = self.controls.iter().find(|item| item.id == incoming.id) {
                    incoming.pending = existing.pending;
                }
            }
        }

        self.controls = controls;

        if let Some(id) = selected_id {
            if let Some(index) = self.controls.iter().position(|item| item.id == id) {
                self.selected_control = index;
                return;
            }
        }

        if self.selected_control >= self.controls.len() {
            self.selected_control = self.controls.len().saturating_sub(1);
        }
    }

    fn mark_control_error(&mut self, id: ControlId, error: String) {
        if let Some(item) = self.controls.iter_mut().find(|item| item.id == id) {
            item.last_error = Some(error);
        }
    }

    fn clear_pending_controls(&mut self) {
        for item in &mut self.controls {
            item.pending = None;
        }
    }

    fn set_message(&mut self, level: MessageLevel, text: impl Into<String>) {
        self.message = StatusMessage {
            level,
            text: text.into(),
        };
    }

    pub(crate) fn selected_control(&self) -> Option<&ControlItem> {
        self.controls.get(self.selected_control)
    }

    pub(crate) fn selected_rgb_field(&self) -> RgbField {
        RgbField::ALL[self.selected_rgb_field]
    }

    pub(crate) fn context_hint(&self) -> String {
        match self.focus {
            FocusPanel::Controls => self.controls_context(),
            FocusPanel::Rgb => self.rgb_context(),
            FocusPanel::Sensors => {
                "Sparklines show rolling sensor history | r refresh sensors".to_string()
            }
        }
    }

    fn controls_context(&self) -> String {
        let Some(item) = self.selected_control() else {
            return "No controls detected | r refresh | q quit".to_string();
        };

        if let Some(choice) = item.pending_choice() {
            return format!(
                "{} preview: {} | Enter apply | Esc cancel",
                item.label(),
                choice.label
            );
        }

        match &item.kind {
            ControlKind::Toggle => {
                format!("{} | Enter toggle | {}", item.label(), item.description())
            }
            ControlKind::Choice(choices) => {
                let mut labels = choices
                    .iter()
                    .take(4)
                    .map(|choice| choice.label.as_str())
                    .collect::<Vec<_>>()
                    .join(" / ");
                if choices.len() > 4 {
                    labels.push_str(" / ...");
                }
                format!(
                    "{} | Left/Right [{labels}] | Enter apply | {}",
                    item.label(),
                    item.description()
                )
            }
        }
    }

    fn rgb_context(&self) -> String {
        let field = self.selected_rgb_field();
        let dirty = if self.rgb_dirty {
            " | unsaved preview"
        } else {
            ""
        };
        format!(
            "{} | Left/Right adjust | Enter apply to keyboard{}",
            field.label(),
            dirty
        )
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.hardware.send(HardwareRequest::Shutdown);
    }
}
