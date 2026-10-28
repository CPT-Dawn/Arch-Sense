//! The Lighting section, driven from the keyboard: effect-dependent rows, the
//! palette and custom hex colours, the debounced live preview, the keyboard
//! preview, and Saved Lighting.

mod common;

use std::time::Duration;

use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::{Packet, RecordingTransport};
use arch_sense::lighting::{Colour, Lighting, LightingBackend, Rgb, saved};
use crossterm::event::KeyCode;
use ratatui::style::Color;

use common::Harness;

const PREAMBLE: Packet = [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E];
/// Shorter than any sensible debounce.
const A_MOMENT: Duration = Duration::from_millis(60);
/// Longer than the debounce.
const SETTLED: Duration = Duration::from_millis(400);

struct Lit {
    _fake: FakeMachine,
    app: Harness,
    keyboard: RecordingTransport,
}

/// Starts the app on a Predator with the USB HID keyboard, `saved` as its
/// Saved Lighting, and the selection on the Lighting section's first row.
fn start(saved: Option<Lighting>) -> Lit {
    let fake = FakeMachine::builder()
        .predator_sense()
        .usb_hid_keyboard()
        .build()
        .expect("fake machine");
    if let Some(lighting) = saved {
        saved::save(&fake.machine(), &lighting).expect("save");
    }
    let keyboard = RecordingTransport::default();
    let mut app = Harness::start(&fake, keyboard.clone());
    app.wait_until("the Lighting section", |app| {
        app.setting_labels().contains(&"Effect".to_string())
    });
    while app.selected_setting().as_deref() != Some("Effect") {
        app.press(KeyCode::Tab);
    }
    Lit {
        _fake: fake,
        app,
        keyboard,
    }
}

fn with_effect(name: &str) -> Lighting {
    let backend = LightingBackend::UsbHid;
    Lighting {
        effect: backend.effect(name).expect(name),
        ..backend.default_lighting()
    }
}

fn lighting_rows(app: &mut Harness) -> Vec<String> {
    let labels = app.setting_labels();
    let start = labels
        .iter()
        .position(|l| l == "Effect")
        .expect("Effect row");
    labels[start..].to_vec()
}

fn select(app: &mut Harness, label: &str) {
    for _ in 0..20 {
        if app.selected_setting().as_deref() == Some(label) {
            return;
        }
        app.press(KeyCode::Down);
    }
    panic!("no {label} row");
}

impl Lit {
    fn wait_for_footer(&mut self, text: &str) {
        self.app.wait_until(text, |app| app.screen().contains(text));
    }
}

#[test]
fn each_effect_shows_only_the_settings_it_uses() {
    let cases: [(&str, &[&str]); 5] = [
        ("Static", &["Effect", "Colour", "Brightness"]),
        ("Breathing", &["Effect", "Colour", "Brightness", "Speed"]),
        ("Wave", &["Effect", "Brightness", "Speed", "Direction"]),
        ("Rainbow", &["Effect", "Brightness", "Speed"]),
        ("Off", &["Effect"]),
    ];
    for (effect, rows) in cases {
        let mut lit = start(Some(with_effect(effect)));

        assert_eq!(lit.app.highlighted("Effect").as_deref(), Some(effect));
        assert_eq!(lighting_rows(&mut lit.app), rows, "{effect}");
    }
}

#[test]
fn changing_the_effect_shows_and_hides_rows_at_once() {
    let mut lit = start(Some(with_effect("Breathing")));

    lit.app.press(KeyCode::Right); // Breathing → Wave

    assert_eq!(lit.app.highlighted("Effect").as_deref(), Some("Wave"));
    assert_eq!(
        lighting_rows(&mut lit.app),
        ["Effect", "Brightness", "Speed", "Direction"]
    );
}

#[test]
fn saved_lighting_is_restored_on_start() {
    let backend = LightingBackend::UsbHid;
    let mut lit = start(Some(Lighting {
        effect: backend.effect("Wave").unwrap(),
        brightness: 85,
        direction: "Clockwise",
        ..backend.default_lighting()
    }));

    assert_eq!(
        lit.app.highlighted("Direction").as_deref(),
        Some("Clockwise")
    );
    assert!(lit.app.row("Brightness").unwrap().contains("85%"));
    assert!(lit.keyboard.sent().is_empty(), "starting doesn't write");
}

#[test]
fn rapid_adjustments_make_one_debounced_write() {
    let mut lit = start(None); // Static, White, 30%
    select(&mut lit.app, "Brightness");

    for _ in 0..5 {
        lit.app.press(KeyCode::Right);
        lit.app.advance(A_MOMENT);
    }
    assert!(
        lit.app.row("Brightness").unwrap().contains("55%"),
        "shown at once"
    );
    assert!(
        lit.keyboard.sent().is_empty(),
        "nothing written while adjusting"
    );

    lit.app.advance(SETTLED);
    lit.wait_for_footer("✓ Brightness → 55% · saved");

    assert_eq!(
        lit.keyboard.sent(),
        vec![vec![
            PREAMBLE,
            [0x14, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00],
            [0x08, 0x02, 0x01, 0x05, 0x1B, 0x01, 0x01, 0x9B],
        ]]
    );
    let saved = saved::load(&lit._fake.machine()).unwrap().unwrap();
    assert_eq!(saved.brightness, 55);
}

#[test]
fn quitting_straight_after_a_change_still_writes_it() {
    let mut lit = start(None);
    select(&mut lit.app, "Brightness");

    lit.app.press(KeyCode::Right);
    lit.app.press(KeyCode::Char('q'));
    drop(lit.app);

    assert_eq!(lit.keyboard.sent().len(), 1);
    let saved = saved::load(&lit._fake.machine()).unwrap().unwrap();
    assert_eq!(saved.brightness, 35);
}

#[test]
fn arrows_step_through_the_palette_and_random() {
    let mut lit = start(None); // White, the last palette colour
    select(&mut lit.app, "Colour");
    assert!(lit.app.row("Colour").unwrap().contains("White"));

    lit.app.press(KeyCode::Right);
    assert!(lit.app.row("Colour").unwrap().contains("Random"));
    lit.app.press(KeyCode::Right);
    assert!(lit.app.row("Colour").unwrap().contains("Red"));
    lit.app.press(KeyCode::Left);
    lit.app.press(KeyCode::Left);
    assert!(lit.app.row("Colour").unwrap().contains("White"));
}

/// The colours of the cells drawn with `glyph` in the current frame, below
/// the gauge cards (whose sparklines use block glyphs too).
fn colours_of(app: &mut Harness, glyph: &str) -> Vec<Color> {
    const BELOW_CARDS: u16 = 6;
    let buffer = app.frame();
    let area = buffer.area;
    (BELOW_CARDS..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .map(|position| &buffer[position])
        .filter(|cell| cell.symbol() == glyph)
        .map(|cell| cell.fg)
        .collect()
}

#[test]
fn a_custom_hex_colour_shows_a_live_swatch_and_applies_on_enter() {
    let mut lit = start(None);
    select(&mut lit.app, "Colour");

    lit.app.press(KeyCode::Enter);
    assert!(lit.app.screen().contains("Custom colour"));
    for key in "12ab3".chars() {
        lit.app.press(KeyCode::Char(key));
    }
    let swatch = Color::Rgb(0x12, 0xAB, 0x34);
    assert!(
        !colours_of(&mut lit.app, "█").contains(&swatch),
        "no swatch until six digits"
    );
    lit.app.press(KeyCode::Enter); // five digits: not yet
    assert!(lit.app.screen().contains("Custom colour"));

    for key in ['g', '4', '5'] {
        lit.app.press(KeyCode::Char(key)); // g isn't hex, 5 is a seventh digit
    }
    assert!(lit.app.screen().contains("12AB34"));
    assert!(colours_of(&mut lit.app, "█").contains(&swatch));

    lit.app.press(KeyCode::Enter);
    assert!(!lit.app.screen().contains("Custom colour"));
    assert!(lit.app.row("Colour").unwrap().contains("#12AB34"));
    lit.app.advance(SETTLED);
    lit.wait_for_footer("✓ Colour → #12AB34 · saved");
    assert_eq!(
        lit.keyboard.sent(),
        vec![vec![
            PREAMBLE,
            [0x14, 0x00, 0x00, 0x12, 0xAB, 0x34, 0x00, 0x00],
            [0x08, 0x02, 0x01, 0x05, 0x0F, 0x01, 0x01, 0x9B],
        ]]
    );
    assert_eq!(
        saved::load(&lit._fake.machine()).unwrap().unwrap().colours,
        vec![Colour::Rgb(Rgb::new(0x12, 0xAB, 0x34))]
    );
}

#[test]
fn backspace_edits_the_hex_input() {
    let mut lit = start(None);
    select(&mut lit.app, "Colour");

    lit.app.press(KeyCode::Char('#'));
    for key in "ff00001".chars() {
        lit.app.press(KeyCode::Char(key));
    }
    lit.app.press(KeyCode::Backspace);
    lit.app.press(KeyCode::Char('9'));
    lit.app.press(KeyCode::Enter);

    assert!(lit.app.row("Colour").unwrap().contains("#FF0009"));
}

#[test]
fn escape_cancels_the_hex_input_without_writing() {
    let mut lit = start(None);
    select(&mut lit.app, "Colour");

    lit.app.press(KeyCode::Enter);
    for key in "ff0000".chars() {
        lit.app.press(KeyCode::Char(key));
    }
    lit.app.press(KeyCode::Esc);
    lit.app.advance(SETTLED);
    lit.app.advance(SETTLED);

    assert!(!lit.app.screen().contains("Custom colour"));
    assert!(lit.app.row("Colour").unwrap().contains("White"));
    assert!(lit.keyboard.sent().is_empty());
    assert_eq!(saved::load(&lit._fake.machine()).unwrap(), None);
}

#[test]
fn the_keyboard_preview_is_pinned_only_while_in_lighting() {
    let backend = LightingBackend::UsbHid;
    let red = Rgb::new(255, 70, 70);
    let mut lit = start(Some(Lighting {
        colours: vec![Colour::Rgb(red)],
        brightness: 100,
        ..backend.default_lighting()
    }));

    let keys = colours_of(&mut lit.app, "▆");
    assert!(
        keys.len() > 40,
        "a keyboard's worth of keys: {}",
        keys.len()
    );
    assert!(
        keys.iter().all(|&c| c == Color::Rgb(red.r, red.g, red.b)),
        "static red at full brightness"
    );

    lit.app.press(KeyCode::Tab); // wraps to Performance
    assert!(colours_of(&mut lit.app, "▆").is_empty());
}

#[test]
fn the_preview_follows_changes_before_the_keyboard_does() {
    let mut lit = start(None);
    select(&mut lit.app, "Colour");
    lit.app.press(KeyCode::Right); // White → Random
    lit.app.press(KeyCode::Right); // → Red

    let red = Color::Rgb(255, 70, 70);
    let dimmed = colours_of(&mut lit.app, "▆");
    assert!(!dimmed.is_empty());
    assert!(dimmed.iter().all(|&c| c != Color::Rgb(255, 255, 255)));
    assert!(dimmed.iter().all(|&c| c != red), "dimmed to 30%");
    assert!(lit.keyboard.sent().is_empty());
}

#[test]
fn clicking_a_swatch_sets_that_colour() {
    let mut lit = start(None); // White
    let (x, y) = lit.app.find_in_row("Colour", "█").expect("first swatch");

    lit.app.click(x, y);

    assert!(lit.app.row("Colour").unwrap().contains("Red"));
    assert_eq!(lit.app.selected_setting().as_deref(), Some("Colour"));
    lit.app.advance(SETTLED);
    lit.wait_for_footer("✓ Colour → Red · saved");
}

#[test]
fn clicking_custom_opens_the_hex_input() {
    let mut lit = start(None);
    let (x, y) = lit.app.find_in_row("Colour", "# custom").expect("custom");

    lit.app.click(x, y);

    assert!(lit.app.screen().contains("Custom colour"));
}

#[test]
fn clicking_an_effect_or_direction_picks_it() {
    let mut lit = start(None); // Static
    let (x, y) = lit
        .app
        .find_in_row("Effect", "Breathing")
        .expect("Breathing");
    lit.app.click(x, y);
    assert_eq!(lit.app.highlighted("Effect").as_deref(), Some("Breathing"));

    lit.app.press(KeyCode::Right); // → Wave
    let (x, y) = lit.app.find_in_row("Direction", "Down").expect("Down");
    lit.app.click(x, y);
    assert_eq!(lit.app.highlighted("Direction").as_deref(), Some("Down"));
}
