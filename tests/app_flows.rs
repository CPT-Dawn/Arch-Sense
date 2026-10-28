mod common;

use std::fs;
use std::time::Duration;

use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::RecordingTransport;
use arch_sense::lighting::saved;
use crossterm::event::KeyCode;

use common::Harness;

#[test]
fn changing_the_thermal_profile_writes_it_at_once_and_confirms_in_the_footer() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();
    assert_eq!(
        app.highlighted("Thermal Profile").as_deref(),
        Some("Balanced")
    );

    app.press(KeyCode::Right);

    let profile = fake.machine().platform_profile();
    app.wait_until("the profile write", |_| {
        fs::read_to_string(&profile).unwrap() == "performance"
    });
    app.wait_until("the footer confirmation", |app| {
        app.screen().contains("✓ Thermal Profile → Performance")
    });
    assert_eq!(
        app.highlighted("Thermal Profile").as_deref(),
        Some("Performance")
    );
}

#[test]
fn changing_the_lighting_sends_it_to_the_keyboard_and_saves_it() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .usb_hid_keyboard()
        .build()
        .expect("fake machine");
    let keyboard = RecordingTransport::default();
    let mut app = Harness::start(&fake, keyboard.clone());
    app.wait_until("the Lighting section", |app| {
        app.setting_labels().contains(&"Effect".to_string())
    });

    app.press(KeyCode::Tab); // Battery
    app.press(KeyCode::Tab); // System
    app.press(KeyCode::Tab); // Lighting
    assert_eq!(app.selected_setting().as_deref(), Some("Effect"));
    app.press(KeyCode::Right); // Static → Breathing

    app.wait_until("the lighting to reach the keyboard", |_| {
        !keyboard.sent().is_empty()
    });
    // Breathing, White, 30% brightness, 50% speed.
    assert_eq!(
        keyboard.sent(),
        vec![vec![
            [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E],
            [0x14, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00],
            [0x08, 0x02, 0x02, 0x05, 0x0F, 0x01, 0x01, 0x9B],
        ]]
    );
    let machine = fake.machine();
    app.wait_until("the lighting to be saved", |_| {
        saved::path(&machine).exists()
    });
    let saved = saved::load(&machine).unwrap().expect("Saved Lighting");
    assert_eq!(saved.effect.name, "Breathing");
    assert_eq!(saved.brightness, 30);
    app.wait_until("the footer confirmation", |app| {
        app.screen().contains("✓ Effect → Breathing · saved")
    });
}

#[test]
fn a_laptop_without_a_keyboard_has_no_lighting_section() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();

    assert!(!app.screen().contains("LIGHTING"));
    assert!(!app.setting_labels().contains(&"Effect".to_string()));
}

#[test]
fn a_confirmation_fades_once_four_seconds_have_passed_on_the_app_clock() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();

    app.press(KeyCode::Right);
    app.wait_until("the footer confirmation", |app| {
        app.screen().contains("✓ Thermal Profile → Performance")
    });

    app.advance(Duration::from_secs(3));
    assert!(app.screen().contains("✓ Thermal Profile → Performance"));
    app.advance(Duration::from_secs(1));
    assert!(!app.screen().contains("✓ Thermal Profile → Performance"));
}

#[test]
fn an_error_stays_in_the_footer_however_long_it_has_been() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .read_only(|m| m.platform_profile())
        .build()
        .expect("fake machine");
    if fs::write(fake.machine().platform_profile(), "balanced").is_ok() {
        eprintln!("skipped: root ignores read-only files");
        return;
    }
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();

    app.press(KeyCode::Right);
    app.wait_until("the error", |app| {
        app.screen().contains("✗ Couldn't set Thermal Profile")
    });

    app.advance(Duration::from_secs(60));
    assert!(app.screen().contains("✗ Couldn't set Thermal Profile"));
}
