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
    "Fans",
    "Charge limit",
    "Calibrate battery",
    "USB charging",
    "Boot animation",
    "LCD overdrive",
    "Backlight timeout",
];

fn start(laptop: FakeMachineBuilder) -> (FakeMachine, Harness) {
    let fake = laptop.build().expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();
    (fake, app)
}

/// The settings rows on screen.
fn shown_controls(app: &mut Harness) -> BTreeSet<String> {
    app.setting_labels().into_iter().collect()
}

fn controls(labels: &[&str]) -> BTreeSet<String> {
    labels.iter().map(|label| label.to_string()).collect()
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
            "Fans",
            "USB charging",
            "Boot animation",
            "LCD overdrive",
            "Backlight timeout",
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
            "Fans",
            "Charge limit",
            "Calibrate battery",
            "USB charging",
            "Backlight timeout",
        ])
    );
    assert!(!app.screen().contains("Linuwu-Sense isn't loaded"));
}

#[test]
fn a_nitro_v4_shows_every_control_and_is_not_reported_as_missing_the_module() {
    let (_fake, mut app) = start(FakeMachine::builder().nitro_v4());

    assert_eq!(shown_controls(&mut app), controls(&CONTROLS));
    assert!(!app.screen().contains("Linuwu-Sense isn't loaded"));
}

#[test]
fn without_the_module_no_controls_are_offered() {
    let (_fake, mut app) = start(FakeMachine::builder().module_missing());

    assert!(shown_controls(&mut app).is_empty());
    assert!(app.screen().contains("Linuwu-Sense isn't loaded"));
}

#[test]
fn nitro_controls_write_to_the_nitro_sense_interface() {
    let (fake, mut app) = start(FakeMachine::builder().nitro_sense());
    let limiter = fake.machine().nitro_sense_dir().join("battery_limiter");

    app.press(KeyCode::Tab);
    assert_eq!(app.selected_setting().as_deref(), Some("Charge limit"));
    app.press(KeyCode::Enter);

    app.wait_until("the limiter write", |_| {
        std::fs::read_to_string(&limiter).unwrap() == "1"
    });
}

#[test]
fn thermal_profile_options_are_the_firmware_choices() {
    let (fake, mut app) = start(FakeMachine::builder().nitro_v4());

    let row = app.row("Thermal Profile").expect("thermal row");
    for profile in [
        "Low Power",
        "Quiet",
        "Balanced",
        "Balanced Performance",
        "Performance",
    ] {
        assert!(row.contains(profile), "{profile:?} missing from {row:?}");
    }

    let mut chosen = Vec::new();
    for _ in 0..5 {
        app.press(KeyCode::Right);
        chosen.push(app.highlighted("Thermal Profile").unwrap_or_default());
    }

    // Starting from Balanced, Right walks the firmware's list and wraps around.
    assert_eq!(
        chosen,
        [
            "Balanced Performance",
            "Performance",
            "Low Power",
            "Quiet",
            "Balanced",
        ]
    );

    app.press(KeyCode::Right);
    let profile = fake.machine().platform_profile();
    app.wait_until("the profile write", |_| {
        std::fs::read_to_string(&profile).unwrap() == "balanced-performance"
    });
}
