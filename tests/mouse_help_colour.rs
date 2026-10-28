//! Mouse support, the help overlay, Esc, and the colour fallback.

mod common;

use std::time::{Duration, Instant};

use arch_sense::app::App;
use arch_sense::fixture::{FakeMachine, FakeMachineBuilder};
use arch_sense::keyboard::RecordingTransport;
use arch_sense::theme::ColorMode;
use crossterm::event::KeyCode;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

use common::Harness;

fn start(laptop: FakeMachineBuilder) -> (FakeMachine, Harness) {
    let fake = laptop.build().expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();
    (fake, app)
}

fn fan_speed(fake: &FakeMachine) -> String {
    std::fs::read_to_string(fake.machine().predator_sense_attr("fan_speed"))
        .expect("attr reads")
        .trim()
        .to_owned()
}

#[test]
fn clicking_a_segment_sets_it_and_selects_its_row() {
    let (fake, mut app) = start(FakeMachine::builder().predator_sense());
    let (x, y) = app.find_in_row("Fans", "Max").expect("Max segment");

    app.click(x, y);

    assert_eq!(app.selected_setting().as_deref(), Some("Fans"));
    app.wait_until("fans written", |_| fan_speed(&fake) == "100,100");
}

#[test]
fn clicking_a_row_selects_it_and_clicking_the_selected_row_cycles_forward() {
    let (fake, mut app) = start(FakeMachine::builder().predator_sense());
    let label = app.label_position("Fans").expect("fans label");

    app.click(label.0, label.1);
    assert_eq!(app.selected_setting().as_deref(), Some("Fans"));
    assert_eq!(fan_speed(&fake), "0,0");

    app.click(label.0, label.1);
    app.wait_until("fans cycled", |_| fan_speed(&fake) == "100,100");

    app.right_click(label.0, label.1);
    app.wait_until("fans cycled back", |_| fan_speed(&fake) == "0,0");
}

#[test]
fn the_scroll_wheel_moves_the_selection() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());
    let first = app.selected_setting().expect("selection");

    app.scroll_down(10, 10);
    let second = app.selected_setting().expect("selection");
    assert_ne!(first, second);

    app.scroll_up(10, 10);
    assert_eq!(app.selected_setting(), Some(first));
}

#[test]
fn clicks_on_nothing_do_nothing() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());
    let before = app.selected_setting();

    app.click(0, 0);

    assert_eq!(app.selected_setting(), before);
}

#[test]
fn question_mark_opens_help_listing_every_binding_and_esc_closes_it() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());

    app.press(KeyCode::Char('?'));
    let screen = app.screen();

    for binding in [
        "Help",
        "↑ k",
        "↓ j",
        "← h",
        "→ l",
        "Enter",
        "Tab",
        "Shift-Tab",
        "Doctor",
        "Ctrl-C",
        "Esc",
        "click",
        "right-click",
        "scroll",
    ] {
        assert!(screen.contains(binding), "{binding:?} missing:\n{screen}");
    }

    app.press(KeyCode::Esc);
    assert!(!app.screen().contains("Shift-Tab"));
    assert!(app.is_running());
}

#[test]
fn esc_closes_the_calibration_dialog_too() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());
    app.press(KeyCode::Tab);
    app.press_n(KeyCode::Down, 2);
    app.press(KeyCode::Enter);
    assert!(app.screen().contains("Calibrate battery?"));

    app.press(KeyCode::Esc);

    assert!(!app.screen().contains("Calibrate battery?"));
}

fn colours(mode: ColorMode) -> Vec<(Color, Color)> {
    let fake = FakeMachine::builder().predator_sense().build().unwrap();
    let mut app = App::new(fake.machine(), RecordingTransport::default()).unwrap();
    app.set_color_mode(mode);
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).unwrap();
    let started = Instant::now();
    loop {
        app.tick(Duration::from_millis(10));
        terminal.draw(|f| app.render(f)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        if text.contains("Thermal") || started.elapsed() > Duration::from_secs(5) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| (c.fg, c.bg))
        .collect()
}

#[test]
fn truecolor_keeps_rgb_colours() {
    assert!(
        colours(ColorMode::Truecolor)
            .iter()
            .any(|(fg, _)| matches!(fg, Color::Rgb(..)))
    );
}

#[test]
fn without_truecolor_the_palette_has_no_rgb() {
    let cells = colours(ColorMode::Ansi16);
    let rgb = |c: &Color| matches!(c, Color::Rgb(..) | Color::Indexed(_));
    assert!(cells.iter().all(|(fg, bg)| !rgb(fg) && !rgb(bg)));
    assert!(cells.iter().any(|(fg, _)| *fg != Color::Reset));
}

#[test]
fn no_color_draws_no_colour_at_all() {
    assert!(
        colours(ColorMode::None)
            .iter()
            .all(|(fg, bg)| *fg == Color::Reset && *bg == Color::Reset)
    );
}

#[test]
fn colour_mode_comes_from_an_injected_environment_reader() {
    fn detect(pairs: &'static [(&str, &str)]) -> ColorMode {
        ColorMode::detect(|name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        })
    }
    assert_eq!(detect(&[("COLORTERM", "truecolor")]), ColorMode::Truecolor);
    assert_eq!(detect(&[("COLORTERM", "24bit")]), ColorMode::Truecolor);
    assert_eq!(detect(&[]), ColorMode::Ansi16);
    assert_eq!(detect(&[("COLORTERM", "yes")]), ColorMode::Ansi16);
    assert_eq!(
        detect(&[("COLORTERM", "truecolor"), ("NO_COLOR", "1")]),
        ColorMode::None
    );
    // Per no-color.org, an empty NO_COLOR doesn't count.
    assert_eq!(detect(&[("NO_COLOR", "")]), ColorMode::Ansi16);
}
