//! Saved Lighting: a versioned JSON document in the state folder, with v1's config migrated.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use arch_sense::fixture::FakeMachine;
use arch_sense::lighting::{Colour, Lighting, LightingBackend, PALETTE, Rgb, saved};
use serde_json::{Value, json};

fn machine() -> FakeMachine {
    FakeMachine::builder()
        .usb_hid_keyboard()
        .build()
        .expect("fake machine")
}

fn wave() -> Lighting {
    let backend = LightingBackend::UsbHid;
    Lighting {
        effect: backend.effect("Wave").unwrap(),
        colours: vec![Colour::Rgb(Rgb::new(0x12, 0x34, 0xAB))],
        brightness: 85,
        speed: 20,
        direction: "Clockwise",
        ..backend.default_lighting()
    }
}

fn saved_json(fake: &FakeMachine) -> Value {
    let text = fs::read_to_string(saved::path(&fake.machine())).expect("saved file");
    serde_json::from_str(&text).expect("saved JSON")
}

fn write_v1(fake: &FakeMachine, rgb: Value) {
    let state = fake.machine().state_dir();
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("config.json"), json!({ "rgb": rgb }).to_string()).unwrap();
}

#[test]
fn nothing_is_saved_until_something_is() {
    let fake = machine();

    assert_eq!(saved::load(&fake.machine()).expect("loads"), None);
}

#[test]
fn saved_lighting_round_trips() {
    let fake = machine();
    for lighting in [wave(), LightingBackend::UsbHid.default_lighting()] {
        saved::save(&fake.machine(), &lighting).expect("saves");

        assert_eq!(saved::load(&fake.machine()).expect("loads"), Some(lighting));
    }
}

#[test]
fn saved_lighting_is_versioned_json_by_name() {
    let fake = machine();
    saved::save(&fake.machine(), &wave()).expect("saves");

    assert_eq!(
        saved_json(&fake),
        json!({
            "version": 2,
            "backend": "usb-hid",
            "effect": "Wave",
            "colours": ["#1234AB"],
            "brightness": 85,
            "speed": 20,
            "direction": "Clockwise",
        })
    );
    assert_eq!(
        saved::path(&fake.machine()),
        fake.machine().state_dir().join("lighting.json")
    );
}

#[test]
fn a_random_colour_round_trips() {
    let fake = machine();
    let mut stars = wave();
    stars.effect = LightingBackend::UsbHid.effect("Stars").unwrap();
    stars.colours = vec![Colour::Random];

    saved::save(&fake.machine(), &stars).expect("saves");

    assert_eq!(saved_json(&fake)["colours"], json!(["random"]));
    assert_eq!(saved::load(&fake.machine()).unwrap(), Some(stars));
}

#[test]
fn saved_lighting_is_writable_by_the_whole_group() {
    let fake = machine();
    saved::save(&fake.machine(), &wave()).expect("saves");

    let mode = fs::metadata(saved::path(&fake.machine()))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o664);
}

#[test]
fn a_v1_config_migrates_to_the_v2_format_on_first_load() {
    let fake = machine();
    // v1: Breathing, palette colour 3 (Emerald), brightness 40, speed 70, direction 1 (Left).
    write_v1(
        &fake,
        json!({"effect": 2, "color": 3, "brightness": 40, "speed": 70, "direction": 1}),
    );

    let loaded = saved::load(&fake.machine()).expect("loads");

    let backend = LightingBackend::UsbHid;
    let expected = Lighting {
        effect: backend.effect("Breathing").unwrap(),
        colours: vec![Colour::Rgb(Rgb::new(52, 211, 153))],
        brightness: 40,
        speed: 70,
        direction: "Left",
        ..backend.default_lighting()
    };
    assert_eq!(loaded, Some(expected.clone()));
    assert_eq!(saved_json(&fake)["version"], 2);
    assert_eq!(saved_json(&fake)["colours"], json!(["#34D399"]));
    assert_eq!(saved::load(&fake.machine()).unwrap(), Some(expected));
}

#[test]
fn every_v1_effect_and_colour_has_a_v2_counterpart() {
    let v1_effects = [
        "Off",
        "Static",
        "Breathing",
        "Wave",
        "Snake",
        "Ripple",
        "Rainbow",
        "Rain",
        "Lightning",
        "Spot",
        "Stars",
        "Fireball",
        "Snow",
        "Heartbeat",
    ];
    for (index, name) in v1_effects.iter().enumerate() {
        let fake = machine();
        write_v1(
            &fake,
            json!({"effect": index, "color": index % 11, "brightness": 30, "speed": 50, "direction": 0}),
        );

        let loaded = saved::load(&fake.machine()).unwrap().expect("migrated");

        assert_eq!(loaded.effect.name, *name);
        let colour = match index % 11 {
            10 => Colour::Random,
            palette => Colour::Rgb(PALETTE[palette].1),
        };
        assert_eq!(loaded.colours, vec![colour], "v1 colour {}", index % 11);
    }
}

#[test]
fn out_of_range_v1_values_are_clamped() {
    let fake = machine();
    write_v1(
        &fake,
        json!({"effect": 99, "color": 99, "brightness": 140, "speed": 120, "direction": 99}),
    );

    let loaded = saved::load(&fake.machine()).unwrap().expect("migrated");

    assert_eq!(loaded.effect.name, "Heartbeat");
    assert_eq!(loaded.colours, vec![Colour::Random]);
    assert_eq!((loaded.brightness, loaded.speed), (100, 100));
    assert_eq!(loaded.direction, "Counter-CW");
}

#[test]
fn saved_lighting_wins_over_a_leftover_v1_config() {
    let fake = machine();
    write_v1(
        &fake,
        json!({"effect": 2, "color": 3, "brightness": 40, "speed": 70, "direction": 1}),
    );
    saved::save(&fake.machine(), &wave()).unwrap();

    assert_eq!(saved::load(&fake.machine()).unwrap(), Some(wave()));
}

#[test]
fn a_corrupt_file_is_an_error_not_a_silent_reset() {
    let fake = machine();
    fs::create_dir_all(fake.machine().state_dir()).unwrap();
    for garbage in [
        "{ not json",
        r##"{"version": 2, "backend": "usb-hid", "effect": "Disco", "colours": ["#FFFFFF"], "brightness": 1, "speed": 1, "direction": "Left"}"##,
        r#"{"version": 9, "backend": "usb-hid"}"#,
    ] {
        fs::write(saved::path(&fake.machine()), garbage).unwrap();

        assert!(saved::load(&fake.machine()).is_err(), "{garbage}");
    }
}
