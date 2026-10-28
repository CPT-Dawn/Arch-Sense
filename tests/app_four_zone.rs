//! The Four-Zone keyboard in the dashboard: the Zone selector, per-zone colours,
//! "Mixed", and clicks that reach the same places the keys do.

mod common;

use std::fs;
use std::time::Duration;

use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::RecordingTransport;
use arch_sense::lighting::saved;
use crossterm::event::KeyCode;

use common::Harness;

/// Longer than the lighting debounce.
const SETTLED: Duration = Duration::from_millis(400);

struct Lit {
    fake: FakeMachine,
    app: Harness,
    keyboard: RecordingTransport,
}

/// A Predator with a Four-Zone keyboard (static white at 30%), selection on Effect.
fn start() -> Lit {
    let fake = FakeMachine::builder()
        .predator_sense()
        .four_zoned_kb()
        .build()
        .expect("fake machine");
    let keyboard = RecordingTransport::default();
    let mut app = Harness::start(&fake, keyboard.clone());
    app.wait_until("the Lighting section", |app| {
        app.setting_labels().contains(&"Effect".to_string())
    });
    select(&mut app, "Effect");
    Lit {
        fake,
        app,
        keyboard,
    }
}

fn select(app: &mut Harness, label: &str) {
    for key in [KeyCode::Down, KeyCode::Up] {
        for _ in 0..30 {
            if app.selected_setting().as_deref() == Some(label) {
                return;
            }
            app.press(key);
        }
    }
    panic!("no {label} row");
}

fn attribute(fake: &FakeMachine, name: &str) -> String {
    fs::read_to_string(fake.machine().four_zone_kb_dir().join(name)).expect(name)
}

impl Lit {
    /// Lets the debounce pass and waits for `per_zone_mode` to hold `expected`.
    fn wait_for_per_zone(&mut self, expected: &str) {
        self.app.advance(SETTLED);
        let fake = &self.fake;
        self.app
            .wait_until(expected, |_| attribute(fake, "per_zone_mode") == expected);
    }

    fn lighting_rows(&mut self) -> Vec<String> {
        let labels = self.app.setting_labels();
        let start = labels.iter().position(|l| l == "Effect").expect("Effect");
        labels[start..].to_vec()
    }
}

#[test]
fn the_zone_selector_appears_for_static_colour_only() {
    let mut lit = start();
    assert_eq!(
        lit.lighting_rows(),
        ["Effect", "Zone", "Colour", "Brightness"]
    );
    assert_eq!(lit.app.highlighted("Zone").as_deref(), Some("All"));

    lit.app.press(KeyCode::Right); // Static → Breathing
    assert_eq!(lit.lighting_rows(), ["Effect", "Colour", "Brightness"]);

    lit.app.press(KeyCode::Right); // → Neon
    assert_eq!(lit.lighting_rows(), ["Effect", "Brightness", "Speed"]);

    lit.app.press(KeyCode::Right); // → Wave
    assert_eq!(
        lit.lighting_rows(),
        ["Effect", "Brightness", "Speed", "Direction"]
    );
    assert!(lit.app.row("Direction").unwrap().contains("Right"));
}

#[test]
fn a_colour_with_one_zone_selected_changes_only_that_zone() {
    let mut lit = start();
    select(&mut lit.app, "Zone");
    lit.app.press_n(KeyCode::Right, 2); // All → 1 → 2
    assert_eq!(lit.app.highlighted("Zone").as_deref(), Some("2"));

    select(&mut lit.app, "Colour");
    lit.app.press(KeyCode::Right); // White → Red
    assert!(lit.app.row("Colour").unwrap().contains("Red"));

    lit.wait_for_per_zone("ffffff,ff4646,ffffff,ffffff,30\n");
    assert!(lit.keyboard.sent().is_empty(), "Four-Zone never talks USB");
}

#[test]
fn zones_that_differ_show_mixed_under_all_and_their_own_colour_otherwise() {
    let mut lit = start();
    select(&mut lit.app, "Zone");
    lit.app.press_n(KeyCode::Right, 3); // zone 3
    select(&mut lit.app, "Colour");
    lit.app.press(KeyCode::Right); // Red

    select(&mut lit.app, "Zone");
    lit.app.press_n(KeyCode::Left, 3); // back to All
    assert_eq!(lit.app.highlighted("Zone").as_deref(), Some("All"));
    assert!(lit.app.row("Colour").unwrap().contains("Mixed"));

    lit.app.press(KeyCode::Right); // zone 1 is still white
    assert!(lit.app.row("Colour").unwrap().contains("White"));
}

#[test]
fn a_colour_under_all_paints_every_zone_with_one_four_zone_mode_write() {
    let mut lit = start();
    select(&mut lit.app, "Zone");
    lit.app.press(KeyCode::Right);
    select(&mut lit.app, "Colour");
    lit.app.press(KeyCode::Right); // zone 1 → Red
    select(&mut lit.app, "Zone");
    lit.app.press(KeyCode::Left); // All, now Mixed
    select(&mut lit.app, "Colour");

    lit.app.press(KeyCode::Right); // swatch stepping starts from the first colour

    assert!(lit.app.row("Colour").unwrap().contains("Red"));
    lit.app.advance(SETTLED);
    let fake = &lit.fake;
    lit.app.wait_until("four_zone_mode written", |_| {
        attribute(fake, "four_zone_mode") == "0,0,30,0,255,70,70\n"
    });
}

#[test]
fn clicking_a_zone_selects_it() {
    let mut lit = start();
    let (x, y) = lit.app.find_in_row("Zone", "3").expect("zone 3 segment");

    lit.app.click(x, y);

    assert_eq!(lit.app.selected_setting().as_deref(), Some("Zone"));
    assert_eq!(lit.app.highlighted("Zone").as_deref(), Some("3"));
}

#[test]
fn clicking_a_swatch_colours_the_selected_zone() {
    let mut lit = start();
    let (x, y) = lit.app.find_in_row("Zone", "4").expect("zone 4 segment");
    lit.app.click(x, y);
    let (x, y) = lit.app.find_in_row("Colour", "█").expect("first swatch");

    lit.app.click(x, y); // Red

    lit.wait_for_per_zone("ffffff,ffffff,ffffff,ff4646,30\n");
}

#[test]
fn a_custom_hex_colour_goes_to_the_selected_zone_too() {
    let mut lit = start();
    select(&mut lit.app, "Zone");
    lit.app.press(KeyCode::Right); // zone 1
    select(&mut lit.app, "Colour");
    lit.app.press(KeyCode::Char('#'));
    for digit in "00ff80".chars() {
        lit.app.press(KeyCode::Char(digit));
    }
    lit.app.press(KeyCode::Enter);

    lit.wait_for_per_zone("00ff80,ffffff,ffffff,ffffff,30\n");
}

#[test]
fn saved_lighting_remembers_the_zones() {
    let mut lit = start();
    select(&mut lit.app, "Zone");
    lit.app.press(KeyCode::Right);
    select(&mut lit.app, "Colour");
    lit.app.press(KeyCode::Right);
    lit.wait_for_per_zone("ff4646,ffffff,ffffff,ffffff,30\n");
    let machine = lit.fake.machine();
    lit.app.wait_until("Saved Lighting", |_| {
        matches!(saved::load(&machine), Ok(Some(_)))
    });

    let saved = saved::load(&machine).unwrap().unwrap();
    assert_eq!(saved.colours.len(), 4);
    assert_ne!(saved.colours[0], saved.colours[1]);
}
