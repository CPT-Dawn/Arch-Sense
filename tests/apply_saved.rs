use std::fs;

use arch_sense::commands::apply_saved_lighting;
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

    apply_saved_lighting(&fake.machine(), &mut keyboard).expect("apply succeeds");

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

    apply_saved_lighting(&fake.machine(), &mut keyboard).expect("apply succeeds");

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

    apply_saved_lighting(&fake.machine(), &mut keyboard).expect("apply succeeds");

    let per_zone = fake.machine().four_zone_kb_dir().join("per_zone_mode");
    assert_eq!(
        fs::read_to_string(per_zone).unwrap(),
        "ff0000,00ff00,0000ff,ffffff,60\n"
    );
    assert!(keyboard.sent().is_empty());
}

/// Everything under the fake machine's Four-Zone directory, by file name.
fn four_zone_files(fake: &FakeMachine) -> Vec<(String, String)> {
    let mut files: Vec<_> = fs::read_dir(fake.machine().four_zone_kb_dir())
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&path).unwrap())
        })
        .collect();
    files.sort();
    files
}

#[test]
fn boot_restore_matches_applying_in_the_ui_on_usb_hid() {
    use arch_sense::lighting::{self, LightingBackend, saved};

    let backend = LightingBackend::UsbHid;
    let lighting = arch_sense::lighting::Lighting {
        effect: backend.effect("Wave").unwrap(),
        brightness: 70,
        speed: 40,
        ..backend.default_lighting()
    };
    let build = || {
        FakeMachine::builder()
            .predator_sense()
            .usb_hid_keyboard()
            .build()
            .unwrap()
    };

    // The UI path: the worker calls `lighting::apply` with the chosen Lighting.
    let ui = build();
    let mut ui_keyboard = RecordingTransport::default();
    lighting::apply(&lighting, &ui.machine(), &mut ui_keyboard).unwrap();

    let boot = build();
    saved::save(&boot.machine(), &lighting).unwrap();
    let mut boot_keyboard = RecordingTransport::default();
    apply_saved_lighting(&boot.machine(), &mut boot_keyboard).unwrap();

    assert!(!ui_keyboard.sent().is_empty());
    assert_eq!(boot_keyboard.sent(), ui_keyboard.sent());
}

#[test]
fn boot_restore_matches_applying_in_the_ui_on_four_zone() {
    use arch_sense::lighting::{self, LightingBackend, saved};

    let backend = LightingBackend::FourZone;
    let lighting = arch_sense::lighting::Lighting {
        effect: backend.effect("Wave").unwrap(),
        brightness: 55,
        speed: 80,
        direction: "Left",
        ..backend.default_lighting()
    };
    let build = || {
        FakeMachine::builder()
            .predator_sense()
            .four_zoned_kb()
            .build()
            .unwrap()
    };

    let ui = build();
    lighting::apply(&lighting, &ui.machine(), &mut RecordingTransport::default()).unwrap();

    let boot = build();
    saved::save(&boot.machine(), &lighting).unwrap();
    apply_saved_lighting(&boot.machine(), &mut RecordingTransport::default()).unwrap();

    assert_eq!(four_zone_files(&boot), four_zone_files(&ui));
    let untouched = build();
    assert_ne!(four_zone_files(&boot), four_zone_files(&untouched));
}

#[test]
fn boot_restore_with_nothing_saved_writes_nothing() {
    for four_zone in [false, true] {
        let builder = FakeMachine::builder().predator_sense();
        let fake = if four_zone {
            builder.four_zoned_kb()
        } else {
            builder.usb_hid_keyboard()
        }
        .build()
        .unwrap();
        let before = four_zone.then(|| four_zone_files(&fake));
        let mut keyboard = RecordingTransport::default();

        apply_saved_lighting(&fake.machine(), &mut keyboard).expect("apply succeeds");

        assert!(keyboard.sent().is_empty());
        assert_eq!(four_zone.then(|| four_zone_files(&fake)), before);
    }
}

#[test]
fn boot_restore_survives_a_corrupt_saved_lighting_file() {
    use arch_sense::lighting::saved;

    let fake = FakeMachine::builder()
        .predator_sense()
        .usb_hid_keyboard()
        .build()
        .unwrap();
    let path = saved::path(&fake.machine());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{ this is not json").unwrap();
    let mut keyboard = RecordingTransport::default();

    apply_saved_lighting(&fake.machine(), &mut keyboard)
        .expect("a corrupt file must not turn boot red");

    assert!(keyboard.sent().is_empty());
}
