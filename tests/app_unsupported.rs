//! A keyboard Arch-Sense can't drive yet: an honest panel, a way to help, and no writes.

mod common;

use arch_sense::fixture::{FakeMachine, FakeMachineBuilder};
use arch_sense::keyboard::RecordingTransport;
use arch_sense::lighting::saved;
use crossterm::event::KeyCode;

use common::Harness;

const CONTRIBUTING: &str = "https://github.com/CPT-Dawn/Arch-Sense#contributing";

fn start(
    laptop: FakeMachineBuilder,
    width: u16,
    height: u16,
) -> (FakeMachine, Harness, RecordingTransport) {
    let fake = laptop.build().expect("fake machine");
    let keyboard = RecordingTransport::default();
    let mut app = Harness::start_sized(&fake, keyboard.clone(), width, height);
    app.wait_for_scan();
    (fake, app, keyboard)
}

fn jing_mold() -> FakeMachineBuilder {
    FakeMachine::builder()
        .predator_sense()
        .sensors()
        .product_name("Predator PH16-72")
        .jing_mold_keyboard()
}

#[test]
fn an_unsupported_keyboard_gets_an_explanation_and_a_way_to_help() {
    let (_fake, mut app, _) = start(jing_mold(), 140, 40);

    let screen = app.screen();

    assert!(screen.contains("isn't supported yet"), "{screen}");
    assert!(screen.contains("Jing-Mold 05af:666a"), "{screen}");
    assert!(screen.contains("USB captures"), "{screen}");
    assert!(screen.contains(CONTRIBUTING), "{screen}");
    assert!(!app.setting_labels().contains(&"Effect".to_string()));
}

#[test]
fn the_panel_fits_the_smallest_terminal() {
    let (_fake, mut app, _) = start(jing_mold(), 80, 24);

    let screen = app.screen();

    assert!(screen.contains("isn't supported yet"), "{screen}");
    assert!(screen.contains(CONTRIBUTING), "{screen}");
    assert!(
        app.setting_labels()
            .contains(&"Thermal Profile".to_string())
    );
}

#[test]
fn the_header_names_the_unsupported_keyboard() {
    let (_fake, mut app, _) = start(jing_mold(), 140, 40);

    let header = app.screen().lines().next().unwrap().to_owned();

    assert!(header.contains("Keyboard not supported"), "{header}");
}

#[test]
fn nothing_is_ever_written_to_an_unsupported_keyboard() {
    let (fake, mut app, keyboard) = start(jing_mold(), 140, 40);

    // Poke at everything the dashboard offers, lighting-shaped keys included.
    for key in [
        KeyCode::Down,
        KeyCode::Tab,
        KeyCode::Tab,
        KeyCode::Tab,
        KeyCode::Tab,
        KeyCode::Right,
        KeyCode::Left,
        KeyCode::Char('#'),
        KeyCode::Esc,
    ] {
        app.press(key);
    }
    app.advance(std::time::Duration::from_secs(1));
    drop(app);

    assert!(keyboard.sent().is_empty(), "no USB transactions");
    assert!(
        !fake.machine().four_zone_kb_dir().exists(),
        "no Four-Zone attributes were conjured"
    );
    assert!(
        saved::load(&fake.machine()).unwrap().is_none(),
        "no lighting was saved"
    );
}

#[test]
fn a_laptop_without_any_rgb_keyboard_gets_no_panel() {
    let (_fake, mut app, _) = start(FakeMachine::builder().predator_sense().sensors(), 140, 40);

    let screen = app.screen();

    assert!(!screen.contains("isn't supported yet"), "{screen}");
    assert!(
        screen
            .lines()
            .next()
            .unwrap()
            .contains("No keyboard lighting")
    );
}

#[test]
fn the_header_names_the_lighting_backend() {
    let cases = [
        (
            FakeMachine::builder().predator_sense().four_zoned_kb(),
            "Four-Zone",
        ),
        (
            FakeMachine::builder().predator_sense().usb_hid_keyboard(),
            "USB HID",
        ),
    ];
    for (laptop, name) in cases {
        let (_fake, mut app, _) = start(laptop, 140, 40);

        let header = app.screen().lines().next().unwrap().to_owned();

        assert!(header.contains(name), "{header}");
    }
}
