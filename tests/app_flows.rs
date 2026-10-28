mod common;

use std::fs;

use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::RecordingTransport;
use crossterm::event::KeyCode;

use common::Harness;

#[test]
fn choosing_a_thermal_profile_writes_it_to_the_platform_profile() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_until("controls to load", |app| {
        app.line_with("Thermal Profile")
            .is_some_and(|line| line.contains("Balanced"))
    });

    app.press(KeyCode::Right);
    app.press(KeyCode::Enter);

    let profile = fake.machine().platform_profile();
    app.wait_until("the profile write", |_| {
        fs::read_to_string(&profile).unwrap() == "performance"
    });
    app.wait_until("the row to show the applied profile", |app| {
        app.line_with("Thermal Profile").is_some_and(|line| {
            line.contains("Performance") && !line.contains("PREVIEW") && !line.contains("APPLY")
        })
    });
}

#[test]
fn applying_lighting_sends_the_effect_to_the_keyboard_and_saves_it() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .usb_hid_keyboard()
        .build()
        .expect("fake machine");
    let keyboard = RecordingTransport::default();
    let mut app = Harness::start(&fake, keyboard.clone());
    app.wait_until("the keyboard to be detected", |app| {
        app.screen().contains("Ready")
    });

    app.press(KeyCode::Tab);
    app.press(KeyCode::Enter);

    app.wait_until("the lighting to reach the keyboard", |_| {
        !keyboard.sent().is_empty()
    });
    // Default lighting: Static, White, 30% brightness, 50% speed.
    assert_eq!(
        keyboard.sent(),
        vec![vec![
            [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E],
            [0x14, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00],
            [0x08, 0x02, 0x01, 0x05, 0x0F, 0x01, 0x01, 0x9B],
        ]]
    );
    let saved = fake.machine().state_dir().join("config.json");
    app.wait_until("the lighting to be saved", |_| saved.exists());
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(saved).unwrap()).unwrap();
    assert_eq!(saved["rgb"]["effect"], 1);
    assert_eq!(saved["rgb"]["brightness"], 30);
}

#[test]
fn a_missing_keyboard_is_reported_in_the_footer() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());

    app.wait_until("the keyboard status", |app| {
        app.screen().contains("Keyboard Not Found")
    });
}
