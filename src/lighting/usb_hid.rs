//! The USB HID Lighting Backend's packet protocol (Chicony `04f2:0117`).
//!
//! Every transaction starts with a preamble, then a colour packet when the
//! effect uses a fixed colour, then one effect packet:
//!
//! ```text
//! preamble  B1 00 00 00 00 00 00 4E
//! colour    14 00 00 RR GG BB 00 00
//! effect    08 02 <opcode> <speed 9..1> <brightness 0..32h> <preset> <direction> 9B
//! ```
//!
//! `preset` is 1 for the given colour and 8 for "keyboard picks at random".

use super::{Colour, Effect, Lighting, Uses};
use crate::keyboard::Packet;

const PREAMBLE: Packet = [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E];
/// What "Off" sends: static at zero brightness.
const OFF: Packet = [0x08, 0x02, 0x01, 0x00, 0x00, 0x01, 0x01, 0x9B];
const BRIGHTNESS_MAX: u16 = 0x32;
const SPEED_FASTEST: u8 = 1;
const SPEED_SLOWEST: u8 = 9;
const PRESET_COLOUR: u8 = 0x01;
const PRESET_RANDOM: u8 = 0x08;

pub(super) const DIRECTIONS: [&str; 6] = ["Right", "Left", "Up", "Down", "Clockwise", "Counter-CW"];

const fn effect(name: &'static str, code: u8, colour: bool, direction: bool) -> Effect {
    Effect {
        name,
        code,
        uses: Uses {
            colour,
            brightness: true,
            speed: true,
            direction,
        },
    }
}

pub(super) const EFFECTS: [Effect; 14] = [
    Effect {
        name: "Off",
        code: 0x01,
        uses: Uses {
            colour: false,
            brightness: false,
            speed: false,
            direction: false,
        },
    },
    Effect {
        name: "Static",
        code: 0x01,
        uses: Uses {
            colour: true,
            brightness: true,
            speed: false,
            direction: false,
        },
    },
    effect("Breathing", 0x02, true, false),
    effect("Wave", 0x03, false, true),
    effect("Snake", 0x05, true, false),
    effect("Ripple", 0x06, true, false),
    effect("Rainbow", 0x08, false, false),
    effect("Rain", 0x0A, true, false),
    effect("Lightning", 0x12, true, false),
    effect("Spot", 0x25, true, false),
    effect("Stars", 0x26, true, false),
    effect("Fireball", 0x27, true, false),
    effect("Snow", 0x28, true, false),
    effect("Heartbeat", 0x29, true, false),
];

/// The packets that put `lighting` on the keyboard, in order.
pub(super) fn packets(lighting: &Lighting) -> Vec<Packet> {
    let effect = lighting.effect;
    if effect.name == "Off" {
        return vec![PREAMBLE, OFF];
    }

    let mut packets = vec![PREAMBLE];
    let colour = effect.uses.colour.then(|| lighting.colour());
    if let Some(Colour::Rgb(rgb)) = colour {
        packets.push([0x14, 0x00, 0x00, rgb.r, rgb.g, rgb.b, 0x00, 0x00]);
    }
    let preset = if colour == Some(Colour::Random) {
        PRESET_RANDOM
    } else {
        PRESET_COLOUR
    };
    let direction = if effect.uses.direction {
        DIRECTIONS
            .iter()
            .position(|&direction| direction == lighting.direction)
            .map_or(1, |index| index as u8 + 1)
    } else {
        1
    };
    packets.push([
        0x08,
        0x02,
        effect.code,
        speed_byte(lighting.speed),
        (lighting.brightness.min(100) as u16 * BRIGHTNESS_MAX / 100) as u8,
        preset,
        direction,
        0x9B,
    ]);
    packets
}

/// 0..=100% → 9 (slowest) ..= 1 (fastest).
fn speed_byte(percent: u8) -> u8 {
    if percent >= 100 {
        return SPEED_FASTEST;
    }
    let range = (SPEED_SLOWEST - SPEED_FASTEST) as u16;
    (SPEED_SLOWEST - (percent as u16 * range / 100) as u8).max(SPEED_FASTEST)
}
