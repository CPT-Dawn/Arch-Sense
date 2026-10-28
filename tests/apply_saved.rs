use std::fs;

use arch_sense::commands::apply_saved_config;
use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::RecordingTransport;

#[test]
fn apply_sends_the_saved_lighting_to_the_keyboard() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .usb_hid_keyboard()
        .build()
        .expect("fake machine");
    let state = fake.machine().state_dir();
    fs::create_dir_all(&state).unwrap();
    // Wave (no colour), 100% brightness, 100% speed, direction Left.
    fs::write(
        state.join("config.json"),
        r#"{"rgb":{"effect":3,"color":0,"brightness":100,"speed":100,"direction":1}}"#,
    )
    .unwrap();
    let mut keyboard = RecordingTransport::default();

    apply_saved_config(&fake.machine(), &mut keyboard).expect("apply succeeds");

    assert_eq!(
        keyboard.sent(),
        vec![vec![
            [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E],
            [0x08, 0x02, 0x03, 0x01, 0x32, 0x01, 0x02, 0x9B],
        ]]
    );
}

#[test]
fn apply_without_a_keyboard_quietly_does_nothing() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");
    let mut keyboard = RecordingTransport::default();

    apply_saved_config(&fake.machine(), &mut keyboard).expect("apply succeeds");

    assert!(keyboard.sent().is_empty());
}

#[test]
fn apply_restores_per_zone_colours_on_a_four_zone_keyboard() {
    use arch_sense::lighting::{Colour, LightingBackend, Rgb, saved};

    let fake = FakeMachine::builder()
        .predator_sense()
        .four_zoned_kb()
        .build()
        .expect("fake machine");
    let backend = LightingBackend::FourZone;
    let lighting = arch_sense::lighting::Lighting {
        colours: vec![
            Colour::Rgb(Rgb::new(255, 0, 0)),
            Colour::Rgb(Rgb::new(0, 255, 0)),
            Colour::Rgb(Rgb::new(0, 0, 255)),
            Colour::Rgb(Rgb::WHITE),
        ],
        brightness: 60,
        ..backend.default_lighting()
    };
    saved::save(&fake.machine(), &lighting).unwrap();
    let mut keyboard = RecordingTransport::default();

    apply_saved_config(&fake.machine(), &mut keyboard).expect("apply succeeds");

    let per_zone = fake.machine().four_zone_kb_dir().join("per_zone_mode");
    assert_eq!(
        fs::read_to_string(per_zone).unwrap(),
        "ff0000,00ff00,0000ff,ffffff,60\n"
    );
    assert!(keyboard.sent().is_empty());
}
