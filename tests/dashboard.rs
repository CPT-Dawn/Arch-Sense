//! The dashboard: header, gauge cards, the settings list and the footer.

mod common;

use std::fs;
use std::path::Path;

use arch_sense::fixture::{FakeMachine, FakeMachineBuilder};
use arch_sense::keyboard::RecordingTransport;
use crossterm::event::KeyCode;

use common::Harness;

fn start(laptop: FakeMachineBuilder) -> (FakeMachine, Harness) {
    start_sized(laptop, common::WIDTH, common::HEIGHT)
}

fn start_sized(laptop: FakeMachineBuilder, width: u16, height: u16) -> (FakeMachine, Harness) {
    let fake = laptop.build().expect("fake machine");
    let mut app = Harness::start_sized(&fake, RecordingTransport::default(), width, height);
    app.wait_for_scan();
    (fake, app)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("attribute reads")
}

fn header(app: &mut Harness) -> String {
    app.screen().lines().next().unwrap_or_default().to_owned()
}

#[test]
fn the_header_names_the_laptop_its_sense_interface_and_lighting_backend() {
    let (_fake, mut app) = start(
        FakeMachine::builder()
            .predator_sense()
            .product_name("Predator PH16-71")
            .usb_hid_keyboard(),
    );

    let header = header(&mut app);

    for part in [
        "ARCH-SENSE",
        "Predator PH16-71",
        "Predator Sense",
        "USB HID keyboard",
    ] {
        assert!(header.contains(part), "{part:?} missing from {header:?}");
    }
}

#[test]
fn the_header_reports_a_nitro_without_keyboard_lighting() {
    let (_fake, mut app) = start(FakeMachine::builder().nitro_sense());

    let header = header(&mut app);

    assert!(header.contains("Nitro Sense"), "{header:?}");
    assert!(header.contains("No keyboard lighting"), "{header:?}");
}

#[test]
fn fans_offer_only_auto_and_max_and_write_their_firmware_values() {
    // Narrow, so the row carries no hint text.
    let (fake, mut app) = start_sized(FakeMachine::builder().predator_sense(), 80, 24);
    let fan_speed = fake.machine().predator_sense_attr("fan_speed");
    app.press(KeyCode::Down);
    assert_eq!(app.selected_setting().as_deref(), Some("Fans"));
    let row = app.row("Fans").expect("fans row");
    let options: Vec<&str> = row
        .split("Fans")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(options, ["Auto", "Max"]);

    app.press(KeyCode::Right);
    app.wait_until("fans at max", |_| read(&fan_speed) == "100,100");
    app.wait_until("the confirmation", |app| {
        app.screen().contains("✓ Fans → Max")
    });

    app.press(KeyCode::Right);
    app.wait_until("fans back to auto", |_| read(&fan_speed) == "0,0");
    assert_eq!(app.highlighted("Fans").as_deref(), Some("Auto"));
}

#[test]
fn enter_flips_a_switch() {
    let (fake, mut app) = start(FakeMachine::builder().predator_sense());
    let limiter = fake.machine().predator_sense_attr("battery_limiter");
    app.press(KeyCode::Tab);
    assert_eq!(app.selected_setting().as_deref(), Some("Charge limit"));
    assert!(app.row("Charge limit").unwrap().contains("On"));

    app.press(KeyCode::Enter);

    app.wait_until("the limiter write", |_| read(&limiter) == "0");
    app.wait_until("the confirmation", |app| {
        app.screen().contains("✓ Charge limit → Off")
    });
    assert!(app.row("Charge limit").unwrap().contains("Off"));
}

/// Root writes through read-only files, so permission tests can't fail there.
fn running_as_root() -> bool {
    fs::read_to_string("/proc/self/status").is_ok_and(|status| {
        status
            .lines()
            .find(|line| line.starts_with("Uid:"))
            .and_then(|line| line.split_whitespace().nth(2))
            == Some("0")
    })
}

#[test]
fn a_failed_write_shows_an_inline_error_pointing_at_the_doctor() {
    if running_as_root() {
        eprintln!("skipped: root ignores read-only files");
        return;
    }
    let (fake, mut app) = start(
        FakeMachine::builder()
            .predator_sense()
            .read_only(|m| m.predator_sense_attr("fan_speed")),
    );
    let fan_speed = fake.machine().predator_sense_attr("fan_speed");
    app.press(KeyCode::Down);

    app.press(KeyCode::Right);

    app.wait_until("the inline error", |app| {
        app.row("Fans")
            .is_some_and(|row| row.contains("permission denied") && row.contains("Doctor"))
    });
    assert!(app.screen().contains("Couldn't set Fans"));
    assert_eq!(app.highlighted("Fans").as_deref(), Some("Auto"));
    assert_eq!(read(&fan_speed), "0,0");
}

#[test]
fn battery_calibration_starts_only_after_confirming() {
    let (fake, mut app) = start(FakeMachine::builder().predator_sense());
    let calibration = fake.machine().predator_sense_attr("battery_calibration");
    app.press(KeyCode::Tab);
    app.press_n(KeyCode::Down, 2);
    assert_eq!(app.selected_setting().as_deref(), Some("Calibrate battery"));

    app.press(KeyCode::Enter);
    assert!(app.screen().contains("Calibrate battery?"));
    assert!(app.screen().contains("Keep AC connected"));
    app.press(KeyCode::Char('y'));

    app.wait_until("calibration to start", |_| read(&calibration) == "1");
    app.wait_until("the confirmation", |app| {
        app.screen().contains("✓ Battery calibration started")
    });
    assert!(!app.screen().contains("Calibrate battery?"));
}

#[test]
fn cancelling_battery_calibration_writes_nothing() {
    let (fake, mut app) = start(FakeMachine::builder().predator_sense());
    let calibration = fake.machine().predator_sense_attr("battery_calibration");
    app.press(KeyCode::Tab);
    app.press_n(KeyCode::Down, 2);

    app.press(KeyCode::Enter);
    app.press(KeyCode::Right); // ignored while the dialog is open
    app.press(KeyCode::Esc);
    app.press(KeyCode::Enter);
    app.press(KeyCode::Char('n'));
    app.press(KeyCode::Enter);
    app.press(KeyCode::Char('q')); // closes the dialog, doesn't quit

    assert!(app.is_running());
    assert!(!app.screen().contains("Calibrate battery?"));
    // Let any stray write land before checking nothing was written.
    app.press(KeyCode::Up);
    assert_eq!(app.selected_setting().as_deref(), Some("USB charging"));
    app.press(KeyCode::Right);
    let usb_charging = fake.machine().predator_sense_attr("usb_charging");
    app.wait_until("a later write", |_| read(&usb_charging) == "10");
    assert_eq!(read(&calibration), "0");
}

#[test]
fn tab_and_shift_tab_jump_between_sections() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());
    assert_eq!(app.selected_setting().as_deref(), Some("Thermal Profile"));

    let mut visited = Vec::new();
    for _ in 0..3 {
        app.press(KeyCode::Tab);
        visited.push(app.selected_setting().unwrap_or_default());
    }
    app.press(KeyCode::BackTab);
    visited.push(app.selected_setting().unwrap_or_default());

    assert_eq!(
        visited,
        [
            "Charge limit",
            "Boot animation",
            "Thermal Profile",
            "Boot animation"
        ]
    );
}

#[test]
fn arrows_and_hjkl_move_the_selection_and_stop_at_the_ends() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());

    app.press(KeyCode::Up);
    assert_eq!(app.selected_setting().as_deref(), Some("Thermal Profile"));
    app.press(KeyCode::Char('j'));
    assert_eq!(app.selected_setting().as_deref(), Some("Fans"));
    app.press_n(KeyCode::Down, 20);
    assert_eq!(app.selected_setting().as_deref(), Some("Backlight timeout"));
    app.press(KeyCode::Char('k'));
    assert_eq!(app.selected_setting().as_deref(), Some("LCD overdrive"));
}

#[test]
fn the_list_scrolls_to_keep_the_selection_visible_on_a_short_terminal() {
    let (_fake, mut app) = start_sized(
        FakeMachine::builder().predator_sense().usb_hid_keyboard(),
        80,
        24,
    );
    app.wait_until("the Lighting section", |app| {
        app.screen().contains("LIGHTING")
    });
    assert!(!app.setting_labels().contains(&"Brightness".to_string()));

    let mut selected = Vec::new();
    for _ in 0..12 {
        app.press(KeyCode::Down);
        selected.push(app.selected_setting());
    }
    assert!(
        selected.iter().all(Option::is_some),
        "selection scrolled off screen: {selected:?}"
    );
    // Static lighting has no speed, so Brightness is the last row.
    assert_eq!(app.selected_setting().as_deref(), Some("Brightness"));
    assert!(
        !app.setting_labels()
            .contains(&"Thermal Profile".to_string())
    );

    app.press(KeyCode::Tab); // wraps to Performance
    assert_eq!(app.selected_setting().as_deref(), Some("Thermal Profile"));
    assert!(app.screen().contains("PERFORMANCE"));
}

#[test]
fn a_terminal_smaller_than_80_by_24_asks_to_be_made_bigger() {
    let (_fake, mut app) = start_sized(FakeMachine::builder().predator_sense(), 80, 24);

    app.resize(79, 24);
    assert!(
        app.screen().contains("I need at least 80×24"),
        "{}",
        app.screen()
    );
    assert!(app.setting_labels().is_empty());

    app.resize(80, 23);
    assert!(app.screen().contains("Terminal is 80×23"));

    app.resize(80, 24);
    assert!(!app.screen().contains("I need at least"));
    assert!(
        app.setting_labels()
            .contains(&"Thermal Profile".to_string())
    );
}

/// The layout of variant C, checked line by line.
fn assert_layout(app: &mut Harness, width: usize, height: usize) {
    let screen = app.screen();
    let lines: Vec<&str> = screen.lines().collect();
    assert_eq!(lines.len(), height);

    assert!(lines[0].contains("ARCH-SENSE"), "header: {:?}", lines[0]);
    let card_titles = lines[1];
    let positions: Vec<usize> = ["CPU", "GPU", "Fans", "Battery"]
        .iter()
        .map(|title| card_titles.find(title).expect(title))
        .collect();
    assert!(positions.is_sorted(), "cards out of order: {card_titles:?}");
    assert!(lines[2].contains("52°C") && lines[2].contains("47°C"));
    assert!(lines[2].contains("2400 / 2600 rpm"));

    let heading = |name: &str| {
        lines
            .iter()
            .position(|line| line.trim_start().starts_with(name))
    };
    let headings = ["PERFORMANCE", "BATTERY", "SYSTEM"].map(heading);
    assert!(headings.iter().all(Option::is_some), "{screen}");
    assert!(headings.is_sorted());
    assert!(headings[0].unwrap() > 6, "the list starts below the cards");

    let footer = lines[height - 1];
    assert!(
        footer.trim_start().starts_with("Changes apply instantly"),
        "{footer:?}"
    );
    assert!(footer.trim_end().ends_with("q quit"), "{footer:?}");
    assert_eq!(footer.chars().count(), width);
}

#[test]
fn the_layout_fits_80_by_24() {
    let (_fake, mut app) = start_sized(FakeMachine::builder().predator_sense().sensors(), 80, 24);

    assert_layout(&mut app, 80, 24);
    let screen = app.screen();
    assert!(
        !screen.contains("tab section"),
        "narrow footer trims its hints"
    );
    assert!(
        !screen.contains("Auto follows the Thermal Profile"),
        "no hints when narrow"
    );
}

#[test]
fn a_wide_terminal_shows_hints_and_every_key() {
    let (_fake, mut app) = start_sized(FakeMachine::builder().predator_sense().sensors(), 120, 30);

    assert_layout(&mut app, 120, 30);
    let screen = app.screen();
    assert!(screen.contains("tab section"));
    assert!(
        app.row("Fans")
            .unwrap()
            .trim_end()
            .ends_with("Auto follows the Thermal Profile")
    );
}

#[test]
fn q_quits() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());

    app.press(KeyCode::Char('q'));

    assert!(!app.is_running());
}

#[test]
fn ctrl_c_quits_even_from_a_dialog() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());
    app.press(KeyCode::Tab);
    app.press_n(KeyCode::Down, 2);
    app.press(KeyCode::Enter);

    app.press_with(KeyCode::Char('c'), crossterm::event::KeyModifiers::CONTROL);

    assert!(!app.is_running());
}
