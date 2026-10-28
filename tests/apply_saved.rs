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
