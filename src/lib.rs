pub mod app;
pub mod capabilities;
pub mod cli;
pub mod commands;
pub mod constants;
pub mod demo;
pub mod doctor;
pub mod fixture;
pub mod hardware;
pub mod keyboard;
pub mod lighting;
pub mod machine;
pub mod models;
mod settings;
pub mod theme;
mod ui;

use anyhow::Result;

use app::App;
use keyboard::KeyboardTransport;
use machine::MachineRoot;

/// Runs the TUI against `machine`, sending keyboard lighting through `keyboard`.
pub fn run(machine: MachineRoot, keyboard: impl KeyboardTransport + 'static) -> Result<()> {
    let mut app = App::new(machine, keyboard)?;
    app.set_color_mode(theme::ColorMode::detect(|name| std::env::var(name).ok()));
    let terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
    let result = app.run(terminal);
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    result
}
