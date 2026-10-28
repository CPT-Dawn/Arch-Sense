//! Drives the real `App` against a fake machine: key events in, rendered screen out.

#![allow(dead_code)] // each test binary uses a different slice of the harness

use std::thread;
use std::time::{Duration, Instant};

use arch_sense::app::App;
use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::KeyboardTransport;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

pub const WIDTH: u16 = 140;
pub const HEIGHT: u16 = 40;
const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_STEP: Duration = Duration::from_millis(10);

/// Settings-list rows start their label at this column (after the list margin
/// and the two-column selection marker).
const LABEL_COLUMN: u16 = 3;
/// Labels never run past this column.
const LABEL_END: u16 = 21;

pub struct Harness {
    app: App,
    terminal: Terminal<TestBackend>,
}

impl Harness {
    pub fn start(fake: &FakeMachine, keyboard: impl KeyboardTransport + 'static) -> Self {
        Self::start_sized(fake, keyboard, WIDTH, HEIGHT)
    }

    /// Starts the app in a terminal of `width` × `height` cells.
    pub fn start_sized(
        fake: &FakeMachine,
        keyboard: impl KeyboardTransport + 'static,
        width: u16,
        height: u16,
    ) -> Self {
        let app = App::new(fake.machine(), keyboard).expect("app starts");
        let terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        Self { app, terminal }
    }

    /// Resizes the terminal, as if the user dragged the window.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.terminal.backend_mut().resize(width, height);
    }

    pub fn press(&mut self, code: KeyCode) {
        self.press_with(code, KeyModifiers::NONE);
    }

    pub fn press_with(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.app.handle_key(KeyEvent::new(code, modifiers));
    }

    fn mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
        // Render first: clicks land on what the last frame drew.
        self.frame();
        self.app.handle_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    }

    pub fn click(&mut self, column: u16, row: u16) {
        self.mouse(MouseEventKind::Down(MouseButton::Left), column, row);
    }

    pub fn right_click(&mut self, column: u16, row: u16) {
        self.mouse(MouseEventKind::Down(MouseButton::Right), column, row);
    }

    pub fn scroll_down(&mut self, column: u16, row: u16) {
        self.mouse(MouseEventKind::ScrollDown, column, row);
    }

    pub fn scroll_up(&mut self, column: u16, row: u16) {
        self.mouse(MouseEventKind::ScrollUp, column, row);
    }

    /// Where `needle` first appears on screen, as (column, row).
    pub fn find(&mut self, needle: &str) -> Option<(u16, u16)> {
        let screen = self.screen();
        screen.lines().enumerate().find_map(|(y, line)| {
            line.find(needle)
                .map(|byte| (line[..byte].chars().count() as u16, y as u16))
        })
    }

    /// Where the label of the settings row `label` starts.
    pub fn label_position(&mut self, label: &str) -> Option<(u16, u16)> {
        let buffer = self.frame();
        let y = (0..buffer.area.height)
            .find(|&y| setting_label(&buffer, y).is_some_and(|(found, _)| found == label))?;
        Some((LABEL_COLUMN, y))
    }

    /// Where `needle` appears on the settings row labelled `label`.
    pub fn find_in_row(&mut self, label: &str, needle: &str) -> Option<(u16, u16)> {
        let buffer = self.frame();
        let y = (0..buffer.area.height)
            .find(|&y| setting_label(&buffer, y).is_some_and(|(found, _)| found == label))?;
        let line = row_text(&buffer, y, LABEL_END, buffer.area.width);
        let byte = line.find(needle)?;
        Some((LABEL_END + line[..byte].chars().count() as u16, y))
    }

    /// Presses `code` `times` times.
    pub fn press_n(&mut self, code: KeyCode, times: usize) {
        for _ in 0..times {
            self.press(code);
        }
    }

    /// Whether the app is still running (it stops after a quit key).
    pub fn is_running(&self) -> bool {
        self.app.is_running()
    }

    /// Renders one frame and returns the buffer.
    pub fn frame(&mut self) -> Buffer {
        self.app.tick(POLL_STEP);
        let app = &self.app;
        self.terminal
            .draw(|frame| app.render(frame))
            .expect("render frame");
        self.terminal.backend().buffer().clone()
    }

    /// Renders one frame and returns the screen as text, one line per row.
    pub fn screen(&mut self) -> String {
        let buffer = self.frame();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            text.push_str(&row_text(&buffer, y, 0, buffer.area.width));
            text.push('\n');
        }
        text
    }

    /// The first screen line containing `needle`, if any.
    pub fn line_with(&mut self, needle: &str) -> Option<String> {
        self.screen()
            .lines()
            .find(|line| line.contains(needle))
            .map(str::to_owned)
    }

    /// The labels of the settings-list rows on screen, top to bottom.
    pub fn setting_labels(&mut self) -> Vec<String> {
        let buffer = self.frame();
        (0..buffer.area.height)
            .filter_map(|y| setting_label(&buffer, y).map(|(label, _)| label))
            .collect()
    }

    /// The label of the selected settings row, if it is on screen.
    pub fn selected_setting(&mut self) -> Option<String> {
        let buffer = self.frame();
        (0..buffer.area.height)
            .filter_map(|y| setting_label(&buffer, y))
            .find(|(_, selected)| *selected)
            .map(|(label, _)| label)
    }

    /// The whole screen line of the settings row labelled exactly `label`.
    pub fn row(&mut self, label: &str) -> Option<String> {
        let buffer = self.frame();
        (0..buffer.area.height)
            .find(|&y| setting_label(&buffer, y).is_some_and(|(found, _)| found == label))
            .map(|y| row_text(&buffer, y, 0, buffer.area.width))
    }

    /// The highlighted option in the row labelled `label`: the cells drawn on a
    /// different background from the rest of the row, e.g. the current segment
    /// of a segmented choice.
    pub fn highlighted(&mut self, label: &str) -> Option<String> {
        let buffer = self.frame();
        let y = (0..buffer.area.height)
            .find(|&y| setting_label(&buffer, y).is_some_and(|(found, _)| found == label))?;
        let row_bg = buffer[(LABEL_COLUMN, y)].bg;
        let text: String = (LABEL_END..buffer.area.width)
            .map(|x| &buffer[(x, y)])
            .filter(|cell| cell.bg != row_bg)
            .map(|cell| cell.symbol())
            .collect();
        let text = text.trim().to_owned();
        (!text.is_empty()).then_some(text)
    }

    /// Keeps ticking and rendering until `done` holds, so the hardware worker can catch up.
    pub fn wait_until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
        let started = Instant::now();
        loop {
            self.app.tick(POLL_STEP);
            if done(self) {
                return;
            }
            if started.elapsed() > SETTLE_TIMEOUT {
                panic!(
                    "timed out waiting for {what}; screen was:\n{}",
                    self.screen()
                );
            }
            thread::sleep(POLL_STEP);
        }
    }

    /// Waits for the first hardware scan to fill the settings list.
    pub fn wait_for_scan(&mut self) {
        self.wait_until("the hardware scan", |app| {
            app.screen().contains("ARCH-SENSE") && !app.screen().contains("Scanning")
        });
    }
}

fn row_text(buffer: &Buffer, y: u16, from: u16, to: u16) -> String {
    (from..to).map(|x| buffer[(x, y)].symbol()).collect()
}

/// The label of the settings row on line `y` and whether it is selected, if
/// line `y` is a settings row.
fn setting_label(buffer: &Buffer, y: u16) -> Option<(String, bool)> {
    if buffer.area.width <= LABEL_END {
        return None;
    }
    let lead = row_text(buffer, y, 0, LABEL_COLUMN);
    let selected = match lead.as_str() {
        " ▌ " => true,
        "   " => false,
        _ => return None,
    };
    let label = row_text(buffer, y, LABEL_COLUMN, LABEL_END);
    let label = label.trim_end();
    let starts_with_text = label.chars().next().is_some_and(char::is_alphanumeric);
    starts_with_text.then(|| (label.to_owned(), selected))
}
