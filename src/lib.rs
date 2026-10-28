pub mod app;
pub mod cli;
pub mod commands;
pub mod config;
pub mod constants;
pub mod fixture;
pub mod hardware;
pub mod keyboard;
pub mod machine;
pub mod models;
pub mod permissions;
pub mod theme;
pub mod ui;

use anyhow::Result;

use app::App;
use keyboard::KeyboardTransport;
use machine::MachineRoot;

/// Runs the TUI against `machine`, sending keyboard lighting through `keyboard`.
pub fn run(machine: MachineRoot, keyboard: impl KeyboardTransport + 'static) -> Result<()> {
    let app = App::new(machine, keyboard)?;
    let terminal = ratatui::init();
    let result = app.run(terminal);
    ratatui::restore();
    result
}
