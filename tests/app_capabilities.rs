//! The app shows exactly the Capabilities the laptop has, whatever its Sense Interface.

mod common;

use std::collections::BTreeSet;

use arch_sense::fixture::{FakeMachine, FakeMachineBuilder};
use arch_sense::keyboard::RecordingTransport;
use crossterm::event::KeyCode;

use common::Harness;

/// Every control row the app can show.
const CONTROLS: [&str; 8] = [
    "Thermal Profile",
    "Fan Speed",
    "Battery Limit",
    "Battery Calibration",
    "USB Charging",
    "Boot Animation",
    "LCD Override",
    "Backlight Timeout",
];

fn start(laptop: FakeMachineBuilder) -> (FakeMachine, Harness) {
    let fake = laptop.build().expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_until("the hardware scan", |app| {
        !app.screen().contains("Waiting for hardware controls")
    });
    (fake, app)
}

/// The control rows on screen. A label must stand alone, so the sensor
/// panel's "Fan Speeds" doesn't count as the "Fan Speed" control.
fn shown_controls(app: &mut Harness) -> BTreeSet<&'static str> {
    let screen = app.screen();
    CONTROLS
        .into_iter()
        .filter(|label| {
            screen.match_indices(label).any(|(at, _)| {
                !screen[at + label.len()..].starts_with(|next: char| next.is_alphanumeric())
            })
        })
        .collect()
}

fn controls(labels: &[&'static str]) -> BTreeSet<&'static str> {
    labels.iter().copied().collect()
}

/// The value shown in the Thermal Profile row, with its state tag.
fn thermal_profile_row(app: &mut Harness) -> String {
    let row = app.line_with("Thermal Profile").expect("thermal row");
    let value = row.split("Thermal Profile").nth(1).expect("row value");
    let value = value.split('│').next().unwrap_or_default();
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn a_full_predator_shows_every_control() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_sense());

    assert_eq!(shown_controls(&mut app), controls(&CONTROLS));
}

#[test]
fn a_legacy_predator_hides_the_battery_controls_its_firmware_rejects() {
    let (_fake, mut app) = start(FakeMachine::builder().predator_legacy());

    assert_eq!(
        shown_controls(&mut app),
        controls(&[
            "Thermal Profile",
            "Fan Speed",
            "USB Charging",
            "Boot Animation",
            "LCD Override",
            "Backlight Timeout",
        ])
    );
}

#[test]
fn a_nitro_shows_its_controls_and_is_not_reported_as_missing_the_module() {
    let (_fake, mut app) = start(FakeMachine::builder().nitro_sense());

    assert_eq!(
        shown_controls(&mut app),
        controls(&[
            "Thermal Profile",
            "Fan Speed",
            "Battery Limit",
            "Battery Calibration",
            "USB Charging",
            "Backlight Timeout",
        ])
    );
    assert!(!app.screen().contains("Kernel Module Missing"));
}

#[test]
fn a_nitro_v4_shows_every_control_and_is_not_reported_as_missing_the_module() {
    let (_fake, mut app) = start(FakeMachine::builder().nitro_v4());

    assert_eq!(shown_controls(&mut app), controls(&CONTROLS));
    assert!(!app.screen().contains("Kernel Module Missing"));
}

#[test]
fn without_the_module_no_controls_are_offered() {
    let (_fake, mut app) = start(FakeMachine::builder().module_missing());

    assert!(shown_controls(&mut app).is_empty());
    assert!(app.screen().contains("Kernel Module Missing"));
}

#[test]
fn nitro_controls_write_to_the_nitro_sense_interface() {
    let (fake, mut app) = start(FakeMachine::builder().nitro_sense());
    let limiter = fake.machine().nitro_sense_dir().join("battery_limiter");

    app.press(KeyCode::Down);
    app.wait_until("the Battery Limit row to be selected", |app| {
        app.line_with("▸")
            .is_some_and(|row| row.contains("Battery Limit"))
    });
    app.press(KeyCode::Enter);

    app.wait_until("the limiter write", |_| {
        std::fs::read_to_string(&limiter).unwrap() == "1"
    });
}

#[test]
fn thermal_profile_options_are_the_firmware_choices() {
    let (fake, mut app) = start(FakeMachine::builder().nitro_v4());

    let mut offered = Vec::new();
    for _ in 0..5 {
        app.press(KeyCode::Right);
        offered.push(thermal_profile_row(&mut app));
    }

    // Starting from Balanced, Right walks the firmware's list and wraps around.
    assert_eq!(
        offered,
        [
            "Balanced Performance PREVIEW",
            "Performance PREVIEW",
            "Low Power PREVIEW",
            "Quiet PREVIEW",
            "Balanced PREVIEW",
        ]
    );

    app.press(KeyCode::Left);
    app.press(KeyCode::Left);
    app.press(KeyCode::Left);
    app.press(KeyCode::Left);
    app.press(KeyCode::Enter);
    let profile = fake.machine().platform_profile();
    app.wait_until("the profile write", |_| {
        std::fs::read_to_string(&profile).unwrap() == "balanced-performance"
    });
}
