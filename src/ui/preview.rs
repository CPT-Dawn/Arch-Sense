//! The keyboard preview: a little staggered keyboard that plays the current
//! lighting, pinned below the list while a Lighting row is selected. It shows
//! what was chosen at once, before the debounced write reaches the keyboard.
//!
//! The animations are impressions, not emulations: each effect is drawn by
//! the family it belongs to (steady, breathing, pulsing, rippling, flowing
//! rainbow, or sparkling), so a new backend's effects get a sensible preview
//! without changes here.

use std::time::Duration;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::lighting::{Colour, Lighting, Rgb};
use crate::theme::Theme;

/// Rows the pinned preview takes.
pub(super) const HEIGHT: u16 = 5;
const KEY: &str = "▆▆";
const KEY_WIDTH: usize = 3;
/// Indent of each keyboard row, like real key stagger.
const STAGGER: [usize; 5] = [0, 1, 2, 2, 3];

pub(super) fn draw(frame: &mut Frame, area: Rect, lighting: &Lighting, clock: Duration) {
    let t = clock.as_secs_f64();
    let width = area.width as usize;
    let rows = (area.height as usize).min(STAGGER.len());
    let lines: Vec<Line> = (0..rows)
        .map(|row| {
            let mut spans = vec![Span::raw(" ".repeat(STAGGER[row]))];
            let mut x = STAGGER[row];
            let mut key = 0;
            while x + KEY_WIDTH <= width {
                let across = (x as f64 + 1.0) / width as f64;
                let colour = key_colour(lighting, across, row, key, t);
                spans.push(Span::styled(KEY, Style::new().fg(colour)));
                spans.push(Span::raw(" "));
                x += KEY_WIDTH;
                key += 1;
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

/// The colour of the key `across` (0..1, left to right) the keyboard, at `t` seconds.
fn key_colour(lighting: &Lighting, across: f64, row: usize, key: usize, t: f64) -> Color {
    let effect = lighting.effect;
    let uses = effect.uses;
    if !uses.brightness {
        return Theme::LINE; // Off
    }

    let speed = if uses.speed {
        0.15 + lighting.speed as f64 / 100.0 * 1.6
    } else {
        0.0
    };
    let backwards = matches!(lighting.direction, "Left" | "Up" | "Counter-CW");
    let heading = if uses.direction && backwards {
        -1.0
    } else {
        1.0
    };
    let seed = (row * 131 + key * 31) as u64;
    let zones = lighting.colours.len().max(1);
    let zone = ((across * zones as f64) as usize).min(zones - 1);
    let base = match lighting.colours.get(zone).copied() {
        Some(Colour::Rgb(rgb)) => rgb,
        // The keyboard picks: give every key its own slowly drifting hue.
        Some(Colour::Random) | None => hue(hash(seed) as f64 / 1000.0 + t * 0.05),
    };

    let rgb = if !uses.colour {
        hue(across * -heading + t * speed * 0.35)
    } else {
        match effect.name {
            "Static" => base,
            "Breathing" => scale(base, 0.2 + 0.8 * (0.5 + 0.5 * (t * speed * 3.0).sin())),
            "Heartbeat" => {
                let beat = (t * speed * 2.0).fract();
                let on = beat < 0.1 || (0.2..0.3).contains(&beat);
                scale(base, if on { 1.0 } else { 0.2 })
            }
            "Ripple" | "Zoom" => {
                let distance = (across - 0.5).abs() * 2.0;
                let near = ((t * speed).fract() - distance).abs() < 0.15;
                scale(base, if near { 1.0 } else { 0.15 })
            }
            _ => {
                let lit = hash(seed ^ (t * speed * 4.0) as u64).is_multiple_of(9);
                scale(base, if lit { 1.0 } else { 0.12 })
            }
        }
    };
    let shown = scale(rgb, 0.25 + 0.75 * lighting.brightness as f64 / 100.0);
    Color::Rgb(shown.r, shown.g, shown.b)
}

fn hash(value: u64) -> u64 {
    value.wrapping_mul(2_654_435_761) % 1000
}

fn scale(rgb: Rgb, factor: f64) -> Rgb {
    let factor = factor.clamp(0.0, 1.0);
    let channel = |c: u8| (c as f64 * factor) as u8;
    Rgb::new(channel(rgb.r), channel(rgb.g), channel(rgb.b))
}

/// A fully saturated colour `h` of the way round the colour wheel.
fn hue(h: f64) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let channel = |c: f64| (c * 255.0) as u8;
    Rgb::new(channel(r), channel(g), channel(b))
}
