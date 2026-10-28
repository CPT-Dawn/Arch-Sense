//! The USB HID Lighting Backend: what reaches the keyboard for each Lighting Effect.

use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::{Packet, RecordingTransport};
use arch_sense::lighting::{self, Colour, Lighting, LightingBackend, Rgb};

const PREAMBLE: Packet = [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E];
const TEAL: Rgb = Rgb {
    r: 0x12,
    g: 0x34,
    b: 0x56,
};
const TEAL_PACKET: Packet = [0x14, 0x00, 0x00, 0x12, 0x34, 0x56, 0x00, 0x00];

/// `effect` in teal, at full brightness and full speed, heading Left.
fn lighting(effect: &str) -> Lighting {
    let backend = LightingBackend::UsbHid;
    let mut lighting = backend.default_lighting();
    lighting.effect = backend.effect(effect).expect(effect);
    lighting.colours = vec![Colour::Rgb(TEAL)];
    lighting.brightness = 100;
    lighting.speed = 100;
    lighting.direction = "Left";
    lighting
}

fn sent(lighting: &Lighting) -> Vec<Vec<Packet>> {
    let fake = FakeMachine::builder()
        .usb_hid_keyboard()
        .build()
        .expect("fake machine");
    let mut keyboard = RecordingTransport::default();
    lighting::apply(lighting, &fake.machine(), &mut keyboard).expect("apply succeeds");
    keyboard.sent()
}

/// The effect packet: opcode, speed 1 (fastest), brightness 0x32 (full), colour preset 1.
fn effect_packet(opcode: u8, direction: u8) -> Packet {
    [0x08, 0x02, opcode, 0x01, 0x32, 0x01, direction, 0x9B]
}

#[test]
fn every_effect_sends_the_v1_packets() {
    // name, opcode, uses colour, uses direction
    let effects: [(&str, u8, bool, bool); 13] = [
        ("Static", 0x01, true, false),
        ("Breathing", 0x02, true, false),
        ("Wave", 0x03, false, true),
        ("Snake", 0x05, true, false),
        ("Ripple", 0x06, true, false),
        ("Rainbow", 0x08, false, false),
        ("Rain", 0x0A, true, false),
        ("Lightning", 0x12, true, false),
        ("Spot", 0x25, true, false),
        ("Stars", 0x26, true, false),
        ("Fireball", 0x27, true, false),
        ("Snow", 0x28, true, false),
        ("Heartbeat", 0x29, true, false),
    ];

    for (name, opcode, colour, direction) in effects {
        let mut expected = vec![PREAMBLE];
        if colour {
            expected.push(TEAL_PACKET);
        }
        // Left is the second direction; effects without one always say 1.
        expected.push(effect_packet(opcode, if direction { 0x02 } else { 0x01 }));
        assert_eq!(sent(&lighting(name)), vec![expected], "{name}");
    }
}

#[test]
fn off_sends_static_at_zero_brightness() {
    assert_eq!(
        sent(&lighting("Off")),
        vec![vec![
            PREAMBLE,
            [0x08, 0x02, 0x01, 0x00, 0x00, 0x01, 0x01, 0x9B]
        ]]
    );
}

#[test]
fn brightness_and_speed_map_to_the_hardware_ranges() {
    // (brightness %, speed %) → (brightness byte 0..0x32, speed byte 9 slow ..1 fast)
    let cases = [
        ((0, 0), (0x00, 0x09)),
        ((30, 50), (0x0F, 0x05)),
        ((55, 95), (0x1B, 0x02)),
        ((100, 100), (0x32, 0x01)),
    ];
    for ((brightness, speed), (brightness_byte, speed_byte)) in cases {
        let mut breathing = lighting("Breathing");
        breathing.brightness = brightness;
        breathing.speed = speed;
        assert_eq!(
            sent(&breathing),
            vec![vec![
                PREAMBLE,
                TEAL_PACKET,
                [
                    0x08,
                    0x02,
                    0x02,
                    speed_byte,
                    brightness_byte,
                    0x01,
                    0x01,
                    0x9B
                ]
            ]],
            "brightness {brightness}%, speed {speed}%"
        );
    }
}

#[test]
fn a_random_colour_asks_the_keyboard_to_pick() {
    let mut stars = lighting("Stars");
    stars.colours = vec![Colour::Random];

    assert_eq!(
        sent(&stars),
        vec![vec![
            PREAMBLE,
            [0x08, 0x02, 0x26, 0x01, 0x32, 0x08, 0x01, 0x9B]
        ]]
    );
}

#[test]
fn every_direction_has_its_own_byte() {
    let directions = LightingBackend::UsbHid.directions();
    assert_eq!(
        directions,
        ["Right", "Left", "Up", "Down", "Clockwise", "Counter-CW"]
    );
    for (byte, direction) in (1u8..).zip(directions) {
        let mut wave = lighting("Wave");
        wave.direction = direction;
        assert_eq!(
            sent(&wave),
            vec![vec![PREAMBLE, effect_packet(0x03, byte)]],
            "{direction}"
        );
    }
}
