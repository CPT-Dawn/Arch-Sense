pub mod app;
pub mod cli;
pub mod commands;
pub mod config;
pub mod constants;
pub mod hardware;
pub mod models;
pub mod permissions;
pub mod theme;
pub mod ui;

use anyhow::Result;

use app::App;

pub fn run() -> Result<()> {
    let app = App::new()?;
    let terminal = ratatui::init();
    let result = app.run(terminal);
    ratatui::restore();
    result
}
