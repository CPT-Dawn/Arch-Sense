//! The dashboard's state and input handling.
//!
//! Every change applies live: a key press turns into a [`Change`], which is shown
//! straight away and sent to the hardware worker. The worker's answer either
//! confirms it in the footer or rolls it back with an inline error. The UI thread
//! never touches sysfs or USB itself.
//!
//! Lighting is the one exception to "at once": it shows at once, but reaches the
//! keyboard only after input settles for [`LIGHTING_DEBOUNCE`], so holding an
//! arrow key makes one write rather than dozens. Time here is whatever
//! [`App::tick`] is told has passed, which keeps the debounce testable.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};

use crate::capabilities::{Capability, SenseInterface};
use crate::hardware::{HardwareEvent, HardwareHandle, HardwareRequest, spawn_worker};
use crate::keyboard::{KeyboardTransport, UsbAccess};
use crate::lighting::{Lighting, Rgb, saved};
use crate::machine::MachineRoot;
use crate::models::{FanMode, SensorSnapshot};
use crate::settings::{Change, LightingField, Section, Setting, SettingId, SettingsModel};

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(1);
const HISTORY_LIMIT: usize = 240;
/// How long a confirmation stays in the footer.
const CONFIRMATION_TIME: Duration = Duration::from_secs(4);
/// How long lighting input has to settle before it is written to the keyboard.
pub(crate) const LIGHTING_DEBOUNCE: Duration = Duration::from_millis(200);
/// How many digits a custom hex colour has.
const HEX_DIGITS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MessageLevel {
    Info,
    Success,
    Error,
}

/// The footer's status message.
#[derive(Clone, Debug)]
pub(crate) struct StatusMessage {
    pub(crate) level: MessageLevel,
    pub(crate) text: String,
    pub(crate) since: Instant,
}

/// A modal drawn over the dashboard. While one is open it takes every key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Overlay {
    /// "Calibrate battery?" — only a yes starts it.
    ConfirmCalibration,
    /// Typing a custom colour: the hex digits so far (uppercase, at most six).
    Hex(String),
}

/// Something clickable drawn in the last frame, for mouse hit-testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    /// A whole settings row.
    Row(SettingId),
    /// One option of a row: a segment, a swatch, a switch or a button.
    Option(SettingId, usize),
}

/// The latest sensor readings and their recent history.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sensors {
    pub(crate) cpu_temp: Option<f64>,
    pub(crate) gpu_temp: Option<f64>,
    pub(crate) cpu_fan: Option<f64>,
    pub(crate) gpu_fan: Option<f64>,
    pub(crate) fan_mode: Option<FanMode>,
    pub(crate) cpu_history: VecDeque<u64>,
    pub(crate) gpu_history: VecDeque<u64>,
}

impl Sensors {
    fn update(&mut self, snapshot: &SensorSnapshot) {
        self.cpu_temp = snapshot.cpu_temp.value;
        self.gpu_temp = snapshot.gpu_temp.value;
        self.cpu_fan = snapshot.cpu_fan.value;
        self.gpu_fan = snapshot.gpu_fan.value;
        self.fan_mode = Some(snapshot.cpu_fan_mode);
        push_history(&mut self.cpu_history, self.cpu_temp);
        push_history(&mut self.gpu_history, self.gpu_temp);
    }
}

fn push_history(history: &mut VecDeque<u64>, value: Option<f64>) {
    let Some(value) = value else {
        return;
    };
    history.push_back(value.clamp(0.0, 150.0).round() as u64);
    while history.len() > HISTORY_LIMIT {
        history.pop_front();
    }
}

pub struct App {
    pub(crate) settings: SettingsModel,
    /// The selected row. Kept by identity so it survives rows appearing and vanishing.
    pub(crate) selected: Option<SettingId>,
    pub(crate) overlay: Option<Overlay>,
    pub(crate) message: Option<StatusMessage>,
    pub(crate) sensors: Sensors,
    pub(crate) keyboard: UsbAccess,
    /// The laptop's DMI model name.
    pub(crate) model: Option<String>,
    /// The first settings-list line on screen; render keeps the selection in view.
    pub(crate) scroll: Cell<usize>,
    /// Clickable areas recorded by the last render.
    pub(crate) hits: RefCell<Vec<(Rect, Hit)>>,
    /// The last value each Capability was confirmed to hold, to roll back to.
    confirmed: BTreeMap<Capability, String>,
    /// Writes sent but not yet answered, per Capability.
    in_flight: HashMap<Capability, usize>,
    /// The latest lighting (Saved Lighting at first), shown again whenever a
    /// scan finds its keyboard.
    last_lighting: Option<Lighting>,
    /// Lighting waiting for input to settle, and how long it has been quiet.
    pending_lighting: Option<(Lighting, Duration)>,
    /// What the latest lighting change was, for its confirmation.
    lighting_note: Option<String>,
    /// Time the app has been told has passed; drives the keyboard preview.
    pub(crate) clock: Duration,
    hardware: HardwareHandle,
    snapshot_pending: bool,
    last_snapshot_request: Instant,
    quit: bool,
}

impl App {
    /// Starts the app on `machine`, with keyboard lighting going through `keyboard`.
    /// Hardware I/O runs on a background worker; [`tick`](Self::tick) collects its results.
    pub fn new(machine: MachineRoot, keyboard: impl KeyboardTransport + 'static) -> Result<Self> {
        let (saved_lighting, saved_warning) = match saved::load(&machine) {
            Ok(lighting) => (lighting, None),
            Err(error) => (None, Some(format!("✗ Saved Lighting ignored: {error:#}"))),
        };
        let hardware = spawn_worker(machine, Box::new(keyboard))?;
        let now = Instant::now();

        let mut app = Self {
            settings: SettingsModel::default(),
            last_lighting: saved_lighting,
            pending_lighting: None,
            clock: Duration::ZERO,
            selected: None,
            overlay: None,
            message: None,
            sensors: Sensors::default(),
            keyboard: UsbAccess::NotFound,
            model: None,
            scroll: Cell::new(0),
            hits: RefCell::new(Vec::new()),
            confirmed: BTreeMap::new(),
            in_flight: HashMap::new(),
            lighting_note: None,
            hardware,
            snapshot_pending: false,
            last_snapshot_request: now - SNAPSHOT_INTERVAL,
            quit: false,
        };
        if let Some(warning) = saved_warning {
            app.set_message(MessageLevel::Error, warning);
        }
        app.request_snapshot();
        Ok(app)
    }

    pub fn run(mut self, mut terminal: ratatui::DefaultTerminal) -> Result<()> {
        let mut last_frame = Instant::now();

        while !self.quit {
            let frame_started = Instant::now();
            self.tick(frame_started.saturating_duration_since(last_frame));
            last_frame = frame_started;
            terminal.draw(|frame| self.render(frame))?;

            let timeout = FRAME_INTERVAL.saturating_sub(frame_started.elapsed());
            if event::poll(timeout)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key);
            }
        }

        Ok(())
    }

    /// Whether the app keeps going; false once the user quits.
    pub fn is_running(&self) -> bool {
        !self.quit
    }

    /// Draws the whole UI into `frame`.
    pub fn render(&self, frame: &mut Frame) {
        crate::ui::draw(frame, self);
    }

    /// Applies finished hardware work and schedules the next poll.
    pub fn tick(&mut self, dt: Duration) {
        self.clock += dt;
        self.handle_hardware_events();

        if let Some((_, quiet)) = &mut self.pending_lighting {
            *quiet += dt;
            if *quiet >= LIGHTING_DEBOUNCE {
                self.flush_lighting();
            }
        }

        if self.last_snapshot_request.elapsed() >= SNAPSHOT_INTERVAL {
            self.request_snapshot();
        }
    }

    /// Sends the lighting waiting for input to settle, if any.
    fn flush_lighting(&mut self) {
        if let Some((lighting, _)) = self.pending_lighting.take()
            && let Err(error) = self.hardware.send(HardwareRequest::ApplyLighting(lighting))
        {
            self.set_message(MessageLevel::Error, error.to_string());
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
                    self.model = snapshot.model;
                    self.sensors.update(&snapshot.sensors);
                    self.settings.lighting = snapshot.lighting.map(|backend| {
                        [self.settings.lighting.take(), self.last_lighting.clone()]
                            .into_iter()
                            .flatten()
                            .find(|lighting| lighting.backend == backend)
                            .unwrap_or_else(|| backend.default_lighting())
                    });
                    self.keyboard = snapshot.keyboard;
                    self.settings.laptop = Some(snapshot.laptop);
                    // Keep values with a write in flight: the scan may predate the write.
                    for (capability, value) in snapshot.values {
                        if !self.is_in_flight(capability) {
                            self.settings.values.insert(capability, value.clone());
                        }
                        self.confirmed.insert(capability, value);
                    }
                    self.keep_selection_valid();
                }
                HardwareEvent::Written { capability, value } => {
                    self.land(capability);
                    self.confirmed.insert(capability, value.clone());
                    let id = SettingId::Hardware(capability);
                    self.settings.errors.remove(&id);
                    let text = if capability == Capability::BatteryCalibration {
                        match value.as_str() {
                            "1" => "✓ Battery calibration started, keep AC connected",
                            _ => "✓ Battery calibration stopped",
                        }
                        .to_string()
                    } else {
                        format!(
                            "✓ {} → {}",
                            id.label(),
                            self.settings.describe(capability, &value)
                        )
                    };
                    self.set_message(MessageLevel::Success, text);
                }
                HardwareEvent::WriteFailed { capability, reason } => {
                    self.land(capability);
                    if !self.is_in_flight(capability)
                        && let Some(value) = self.confirmed.get(&capability)
                    {
                        self.settings.values.insert(capability, value.clone());
                    }
                    let id = SettingId::Hardware(capability);
                    self.settings.errors.insert(id, reason.clone());
                    self.set_message(
                        MessageLevel::Error,
                        format!(
                            "✗ Couldn't set {}: {reason}. Press d for the Doctor",
                            id.label()
                        ),
                    );
                }
                HardwareEvent::LightingApplied => {
                    let note = self
                        .lighting_note
                        .take()
                        .unwrap_or_else(|| "Lighting".into());
                    self.set_message(MessageLevel::Success, format!("✓ {note} · saved"));
                }
                HardwareEvent::LightingNotSaved(error) => {
                    self.lighting_note = None;
                    self.set_message(
                        MessageLevel::Error,
                        format!("✗ Lighting applied but not saved: {error}"),
                    );
                }
                HardwareEvent::LightingFailed(error) => {
                    self.lighting_note = None;
                    self.set_message(
                        MessageLevel::Error,
                        format!("✗ Lighting failed: {error}. Press d for the Doctor"),
                    );
                }
            }
        }
    }

    fn is_in_flight(&self, capability: Capability) -> bool {
        self.in_flight
            .get(&capability)
            .is_some_and(|&count| count > 0)
    }

    /// Notes that one write to `capability` got its answer.
    fn land(&mut self, capability: Capability) {
        if let Some(count) = self.in_flight.get_mut(&capability) {
            *count = count.saturating_sub(1);
        }
    }

    /// Reacts to one key press.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        if let Some(overlay) = self.overlay.clone() {
            self.on_overlay_key(overlay, key);
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            KeyCode::Tab => self.jump_section(1),
            KeyCode::BackTab => self.jump_section(-1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Left | KeyCode::Char('h') => self.change_selected(-1),
            KeyCode::Right | KeyCode::Char('l') => self.change_selected(1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(id) = self.selected_id() {
                    let change = self.settings.activate(id);
                    self.make(id, change);
                }
            }
            KeyCode::Char('#') => {
                let id = SettingId::Lighting(LightingField::Colour);
                if self.selected_id() == Some(id) && self.settings.control(id).is_some() {
                    self.make(id, Some(Change::CustomColour));
                }
            }
            _ => {}
        }
    }

    fn on_overlay_key(&mut self, overlay: Overlay, key: KeyEvent) {
        match overlay {
            Overlay::Hex(digits) => self.on_hex_key(digits, key),
            Overlay::ConfirmCalibration => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.overlay = None;
                    let change = Change::Write {
                        capability: Capability::BatteryCalibration,
                        value: "1".to_string(),
                    };
                    self.make(
                        SettingId::Hardware(Capability::BatteryCalibration),
                        Some(change),
                    );
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') | KeyCode::Esc => {
                    self.overlay = None;
                    self.set_message(MessageLevel::Info, "Battery calibration not started");
                }
                _ => {}
            },
        }
    }

    fn on_hex_key(&mut self, mut digits: String, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.overlay = None;
                self.set_message(MessageLevel::Info, "Colour unchanged");
            }
            KeyCode::Backspace => {
                digits.pop();
                self.overlay = Some(Overlay::Hex(digits));
            }
            KeyCode::Char(c) if c.is_ascii_hexdigit() && digits.len() < HEX_DIGITS => {
                digits.push(c.to_ascii_uppercase());
                self.overlay = Some(Overlay::Hex(digits));
            }
            KeyCode::Enter => {
                if let Some(rgb) = Rgb::from_hex(&digits) {
                    self.overlay = None;
                    let change = self.settings.custom_colour(rgb);
                    self.make(SettingId::Lighting(LightingField::Colour), change);
                }
            }
            _ => {}
        }
    }

    fn change_selected(&mut self, step: i8) {
        if let Some(id) = self.selected_id() {
            let change = self.settings.change(id, step);
            self.make(id, change);
        }
    }

    /// Shows `change` at once and sends it to the hardware worker.
    fn make(&mut self, id: SettingId, change: Option<Change>) {
        let Some(change) = change else {
            return;
        };
        let request = match &change {
            Change::ConfirmCalibration => {
                self.overlay = Some(Overlay::ConfirmCalibration);
                return;
            }
            Change::CustomColour => {
                self.overlay = Some(Overlay::Hex(String::new()));
                return;
            }
            Change::Write { capability, value } => {
                *self.in_flight.entry(*capability).or_default() += 1;
                HardwareRequest::Write {
                    capability: *capability,
                    value: value.clone(),
                }
            }
            Change::Lighting(lighting) => {
                self.last_lighting = Some(lighting.clone());
                self.settings.apply(&change);
                if let Some(control) = self.settings.control(id) {
                    self.lighting_note = Some(format!("{} → {}", id.label(), control.value_text()));
                }
                self.pending_lighting = Some((lighting.clone(), Duration::ZERO));
                self.keep_selection_valid();
                return;
            }
        };
        self.settings.apply(&change);
        self.keep_selection_valid();
        if let Err(error) = self.hardware.send(request) {
            self.set_message(MessageLevel::Error, error.to_string());
        }
    }

    /// The visible settings rows, in list order.
    pub(crate) fn rows(&self) -> Vec<Setting> {
        self.settings.rows()
    }

    /// The selected row's id, if any row is visible.
    pub(crate) fn selected_id(&self) -> Option<SettingId> {
        let rows = self.rows();
        self.selected
            .filter(|id| rows.iter().any(|row| row.id == *id))
            .or_else(|| rows.first().map(|row| row.id))
    }

    fn move_selection(&mut self, step: isize) {
        let rows = self.rows();
        let Some(current) = self.selected_id() else {
            return;
        };
        let index = rows.iter().position(|row| row.id == current).unwrap_or(0);
        let next = index.saturating_add_signed(step).min(rows.len() - 1);
        self.selected = Some(rows[next].id);
    }

    /// Moves to the first row of the next (or previous) section that has rows.
    fn jump_section(&mut self, step: isize) {
        let rows = self.rows();
        let Some(current) = self.selected_id() else {
            return;
        };
        let sections: Vec<Section> = Section::ALL
            .into_iter()
            .filter(|section| rows.iter().any(|row| row.id.section() == *section))
            .collect();
        let here = sections
            .iter()
            .position(|section| *section == current.section())
            .unwrap_or(0);
        let next = sections[(here as isize + step).rem_euclid(sections.len() as isize) as usize];
        self.selected = rows
            .iter()
            .find(|row| row.id.section() == next)
            .map(|row| row.id);
    }

    /// After rows appear or vanish, keeps the selection on a visible row,
    /// preferring the row that took the vanished one's place.
    fn keep_selection_valid(&mut self) {
        let rows = self.rows();
        if self
            .selected
            .is_some_and(|id| rows.iter().any(|row| row.id == id))
        {
            return;
        }
        self.selected = self.selected.and_then(|gone| {
            rows.iter()
                .find(|row| row.id.section() == gone.section())
                .map(|row| row.id)
        });
        if self.selected.is_none() {
            self.selected = rows.first().map(|row| row.id);
        }
    }

    fn set_message(&mut self, level: MessageLevel, text: impl Into<String>) {
        self.message = Some(StatusMessage {
            level,
            text: text.into(),
            since: Instant::now(),
        });
    }

    /// The footer's status message, if one is current. Confirmations fade;
    /// errors stay until something replaces them.
    pub(crate) fn status(&self) -> Option<&StatusMessage> {
        self.message.as_ref().filter(|message| {
            message.level == MessageLevel::Error || message.since.elapsed() < CONFIRMATION_TIME
        })
    }

    /// Whether the first hardware scan has landed.
    pub(crate) fn scanned(&self) -> bool {
        self.settings.laptop.is_some()
    }

    /// Whether Linuwu-Sense is loaded with a Sense Interface for this laptop.
    pub(crate) fn module_loaded(&self) -> bool {
        self.settings
            .laptop
            .as_ref()
            .is_some_and(|laptop| laptop.sense_interface != SenseInterface::None)
    }

    /// Records that `area` was drawn for `hit`, for mouse hit-testing.
    pub(crate) fn record_hit(&self, area: Rect, hit: Hit) {
        self.hits.borrow_mut().push((area, hit));
    }

    /// What was drawn at `column`, `row` in the last frame; the topmost wins.
    #[allow(dead_code)] // mouse handling (#12) is the caller
    pub(crate) fn hit_at(&self, column: u16, row: u16) -> Option<Hit> {
        self.hits
            .borrow()
            .iter()
            .rev()
            .find(|(area, _)| area.contains(Position::new(column, row)))
            .map(|(_, hit)| *hit)
    }
}

impl Drop for App {
    /// Writes any lighting still settling, then waits for the worker to finish,
    /// so quitting straight after a change doesn't lose it.
    fn drop(&mut self) {
        self.flush_lighting();
        self.hardware.shutdown();
    }
}
