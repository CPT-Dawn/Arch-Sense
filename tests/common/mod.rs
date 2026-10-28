//! Drives the real `App` against a fake machine: key events in, rendered screen out.

#![allow(dead_code)] // each test binary uses a different slice of the harness

use std::thread;
use std::time::{Duration, Instant};

use arch_sense::app::App;
use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::KeyboardTransport;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

const WIDTH: u16 = 140;
const HEIGHT: u16 = 40;
const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_STEP: Duration = Duration::from_millis(10);

pub struct Harness {
    app: App,
    terminal: Terminal<TestBackend>,
}

impl Harness {
    pub fn start(fake: &FakeMachine, keyboard: impl KeyboardTransport + 'static) -> Self {
        let app = App::new(fake.machine(), keyboard).expect("app starts");
        let terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).expect("test terminal");
        Self { app, terminal }
    }

    pub fn press(&mut self, code: KeyCode) {
        self.app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    /// Renders one frame and returns the screen as text, one line per row.
    pub fn screen(&mut self) -> String {
        self.app.tick(POLL_STEP);
        let app = &self.app;
        self.terminal
            .draw(|frame| app.render(frame))
            .expect("render frame");
        let buffer = self.terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
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

    /// Keeps ticking and rendering until `done` holds, so the hardware worker can catch up.
    pub fn wait_until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
        let started = Instant::now();
        while !done(self) {
            if started.elapsed() > SETTLE_TIMEOUT {
                panic!(
                    "timed out waiting for {what}; screen was:\n{}",
                    self.screen()
                );
            }
            thread::sleep(POLL_STEP);
        }
    }
}
