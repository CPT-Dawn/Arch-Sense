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

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

use app::App;
use keyboard::KeyboardTransport;
use machine::MachineRoot;

/// Runs the TUI against `machine`, sending keyboard lighting through `keyboard`.
pub fn run(machine: MachineRoot, keyboard: impl KeyboardTransport + 'static) -> Result<()> {
    run_app(App::new(machine, keyboard)?)
}

/// Runs `app` on the real terminal. SIGTERM, SIGHUP and SIGINT quit through the
/// normal path, so the terminal is restored and the app (and any demo tree) cleaned up.
pub(crate) fn run_app(mut app: App) -> Result<()> {
    app.set_color_mode(theme::ColorMode::detect(|name| std::env::var(name).ok()));
    let shutdown = Arc::new(AtomicBool::new(false));
    for signal in [SIGTERM, SIGHUP, SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&shutdown))?;
    }
    app.watch_shutdown(shutdown);
    let terminal = ratatui::init();
    // ratatui's panic hook restores raw mode and the alternate screen; add mouse capture.
    let ratatui_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
        ratatui_hook(info);
    }));
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
    let result = app.run(terminal);
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    result
}
