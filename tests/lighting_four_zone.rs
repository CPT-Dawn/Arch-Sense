//! The Four-Zone Lighting Backend: what lands in Linuwu-Sense's `four_zoned_kb`
//! attributes for each Lighting Effect and for per-zone static colours.

use std::fs;

use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::RecordingTransport;
use arch_sense::lighting::{self, Colour, Lighting, LightingBackend, Rgb};

const TEAL: Rgb = Rgb::new(0x12, 0x34, 0x56);
/// What the fixture's attributes hold before anything is written.
const UNTOUCHED_MODE: &str = "0,0,100,0,255,255,255\n";
const UNTOUCHED_PER_ZONE: &str = "ffffff,ffffff,ffffff,ffffff,100\n";

/// `effect` in teal at 80% brightness, full speed, heading Left.
fn lighting(effect: &str) -> Lighting {
    let backend = LightingBackend::FourZone;
    let mut lighting = backend.default_lighting();
    lighting.effect = backend.effect(effect).expect(effect);
    lighting.colours = vec![Colour::Rgb(TEAL); backend.zones()];
    lighting.brightness = 80;
    lighting.speed = 100;
    lighting.direction = "Left";
    lighting
}

/// Applies `lighting` to a Four-Zone laptop and returns what `four_zone_mode`
/// and `per_zone_mode` hold afterwards.
fn applied(lighting: &Lighting) -> (String, String) {
    let fake = FakeMachine::builder()
        .four_zoned_kb()
        .build()
        .expect("fake machine");
    let mut keyboard = RecordingTransport::default();
    lighting::apply(lighting, &fake.machine(), &mut keyboard).expect("apply succeeds");
    assert!(keyboard.sent().is_empty(), "Four-Zone never talks USB");
    let dir = fake.machine().four_zone_kb_dir();
    let read = |name: &str| fs::read_to_string(dir.join(name)).expect(name);
    (read("four_zone_mode"), read("per_zone_mode"))
}

#[test]
fn every_effect_writes_its_four_zone_mode_string() {
    // mode,speed,brightness,direction,red,green,blue
    let cases = [
        ("Static", "0,0,80,0,18,52,86\n"),
        ("Breathing", "1,0,80,0,18,52,86\n"),
        ("Neon", "2,9,80,0,0,0,0\n"),
        ("Wave", "3,9,80,1,0,0,0\n"),
        ("Shifting", "4,9,80,1,18,52,86\n"),
        ("Zoom", "5,9,80,0,18,52,86\n"),
        ("Meteor", "6,9,80,0,18,52,86\n"),
        ("Twinkling", "7,9,80,0,18,52,86\n"),
    ];
    for (effect, expected) in cases {
        let (mode, per_zone) = applied(&lighting(effect));
        assert_eq!(mode, expected, "{effect}");
        assert_eq!(per_zone, UNTOUCHED_PER_ZONE, "{effect}");
    }
}

#[test]
fn static_zones_that_differ_write_the_exact_per_zone_mode_string() {
    let mut lit = lighting("Static");
    lit.colours = vec![
        Colour::Rgb(Rgb::new(255, 0, 0)),
        Colour::Rgb(Rgb::new(0, 255, 0)),
        Colour::Rgb(Rgb::new(0x0a, 0x0b, 0x0c)),
        Colour::Rgb(Rgb::WHITE),
    ];

    let (mode, per_zone) = applied(&lit);

    assert_eq!(per_zone, "ff0000,00ff00,0a0b0c,ffffff,80\n");
    assert_eq!(mode, UNTOUCHED_MODE);
}

#[test]
fn an_effect_that_cannot_show_zones_sends_the_first_zone_colour() {
    let mut lit = lighting("Breathing");
    lit.colours[0] = Colour::Rgb(Rgb::new(1, 2, 3));
    lit.colours[3] = Colour::Rgb(Rgb::WHITE);

    let (mode, per_zone) = applied(&lit);

    assert_eq!(mode, "1,0,80,0,1,2,3\n");
    assert_eq!(per_zone, UNTOUCHED_PER_ZONE);
}

#[test]
fn directions_map_to_the_driver_values() {
    let mut wave = lighting("Wave");
    wave.direction = "Right";
    assert_eq!(applied(&wave).0, "3,9,80,2,0,0,0\n");
    wave.direction = "Left";
    assert_eq!(applied(&wave).0, "3,9,80,1,0,0,0\n");
}

#[test]
fn speed_spans_the_drivers_zero_to_nine() {
    let mut neon = lighting("Neon");
    for (percent, expected) in [
        (0, "2,0,80,0,0,0,0\n"),
        (50, "2,5,80,0,0,0,0\n"),
        (100, "2,9,80,0,0,0,0\n"),
    ] {
        neon.speed = percent;
        assert_eq!(applied(&neon).0, expected, "{percent}%");
    }
}
