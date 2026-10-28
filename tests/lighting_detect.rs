//! Lighting Backend detection: Four-Zone first, then USB HID, then a keyboard
//! we know but can't drive yet, then nothing.

use arch_sense::fixture::FakeMachine;
use arch_sense::lighting::{self, Detection, LightingBackend};

fn detect(builder: arch_sense::fixture::FakeMachineBuilder) -> Detection {
    let fake = builder.build().expect("fake machine");
    lighting::detect(&fake.machine())
}

#[test]
fn four_zone_is_found_through_linuwu_sense() {
    let detection = detect(FakeMachine::builder().predator_sense().four_zoned_kb());

    assert_eq!(detection, Detection::Backend(LightingBackend::FourZone));
}

#[test]
fn usb_hid_is_found_by_its_usb_id() {
    let detection = detect(FakeMachine::builder().predator_sense().usb_hid_keyboard());

    assert_eq!(detection, Detection::Backend(LightingBackend::UsbHid));
}

#[test]
fn four_zone_wins_when_both_are_present() {
    let detection = detect(FakeMachine::builder().usb_hid_keyboard().four_zoned_kb());

    assert_eq!(detection, Detection::Backend(LightingBackend::FourZone));
}

#[test]
fn a_known_unsupported_keyboard_is_named() {
    let detection = detect(FakeMachine::builder().predator_sense().jing_mold_keyboard());

    let Detection::Unsupported(keyboard) = detection else {
        panic!("expected Unsupported, got {detection:?}");
    };
    assert_eq!((keyboard.vendor, keyboard.product), (0x05af, 0x666a));
    assert_eq!(keyboard.name, "Jing-Mold");
    assert_eq!(detection.backend(), None);
}

#[test]
fn a_supported_backend_beats_an_unsupported_keyboard() {
    let detection = detect(FakeMachine::builder().jing_mold_keyboard().four_zoned_kb());

    assert_eq!(detection, Detection::Backend(LightingBackend::FourZone));
}

#[test]
fn no_keyboard_at_all_is_none() {
    let detection = detect(FakeMachine::builder().predator_sense());

    assert_eq!(detection, Detection::None);
}
