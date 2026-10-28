use crate::config::RgbConfig;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SensorMetric {
    pub(crate) value: Option<f64>,
    pub(crate) error: Option<String>,
}

impl SensorMetric {
    pub(crate) fn available(value: f64) -> Self {
        Self {
            value: Some(value),
            error: None,
        }
    }

    pub(crate) fn unavailable(error: impl Into<String>) -> Self {
        Self {
            value: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FanMode {
    Auto,
    Max,
}

impl FanMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Max => "Max",
        }
    }
}

/// What the power-supply class says about the battery.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Battery {
    pub(crate) percent: u8,
    pub(crate) state: BatteryState,
    /// Power flowing in or out, when the battery reports it.
    pub(crate) watts: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatteryState {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl BatteryState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "On battery",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SensorSnapshot {
    pub(crate) cpu_temp: SensorMetric,
    pub(crate) gpu_temp: SensorMetric,
    pub(crate) cpu_fan: SensorMetric,
    pub(crate) gpu_fan: SensorMetric,
    pub(crate) cpu_fan_mode: FanMode,
    pub(crate) gpu_fan_mode: FanMode,
    /// `None` when the laptop has no battery.
    pub(crate) battery: Option<Battery>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RgbField {
    Effect,
    Color,
    Brightness,
    Speed,
    Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rgb {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColorDef {
    pub(crate) name: &'static str,
    pub(crate) rgb: Rgb,
}

pub(crate) const COLOR_PALETTE: [ColorDef; 11] = [
    ColorDef {
        name: "Red",
        rgb: Rgb {
            r: 255,
            g: 70,
            b: 70,
        },
    },
    ColorDef {
        name: "Orange",
        rgb: Rgb {
            r: 255,
            g: 142,
            b: 45,
        },
    },
    ColorDef {
        name: "Gold",
        rgb: Rgb {
            r: 250,
            g: 204,
            b: 21,
        },
    },
    ColorDef {
        name: "Emerald",
        rgb: Rgb {
            r: 52,
            g: 211,
            b: 153,
        },
    },
    ColorDef {
        name: "Cyan",
        rgb: Rgb {
            r: 34,
            g: 211,
            b: 238,
        },
    },
    ColorDef {
        name: "Blue",
        rgb: Rgb {
            r: 96,
            g: 165,
            b: 250,
        },
    },
    ColorDef {
        name: "Violet",
        rgb: Rgb {
            r: 167,
            g: 139,
            b: 250,
        },
    },
    ColorDef {
        name: "Magenta",
        rgb: Rgb {
            r: 232,
            g: 121,
            b: 249,
        },
    },
    ColorDef {
        name: "Pink",
        rgb: Rgb {
            r: 244,
            g: 114,
            b: 182,
        },
    },
    ColorDef {
        name: "White",
        rgb: Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
    },
    ColorDef {
        name: "Random",
        rgb: Rgb { r: 0, g: 0, b: 0 },
    },
];

pub(crate) const RANDOM_COLOR_INDEX: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RgbEffect {
    pub(crate) name: &'static str,
    pub(crate) opcode: u8,
    pub(crate) has_color: bool,
    pub(crate) has_direction: bool,
}

pub(crate) const RGB_EFFECTS: [RgbEffect; 14] = [
    RgbEffect {
        name: "Off",
        opcode: 0x01,
        has_color: false,
        has_direction: false,
    },
    RgbEffect {
        name: "Static",
        opcode: 0x01,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Breathing",
        opcode: 0x02,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Wave",
        opcode: 0x03,
        has_color: false,
        has_direction: true,
    },
    RgbEffect {
        name: "Snake",
        opcode: 0x05,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Ripple",
        opcode: 0x06,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Rainbow",
        opcode: 0x08,
        has_color: false,
        has_direction: false,
    },
    RgbEffect {
        name: "Rain",
        opcode: 0x0A,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Lightning",
        opcode: 0x12,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Spot",
        opcode: 0x25,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Stars",
        opcode: 0x26,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Fireball",
        opcode: 0x27,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Snow",
        opcode: 0x28,
        has_color: true,
        has_direction: false,
    },
    RgbEffect {
        name: "Heartbeat",
        opcode: 0x29,
        has_color: true,
        has_direction: false,
    },
];

pub(crate) const OFF_EFFECT_INDEX: usize = 0;
pub(crate) const DIRECTIONS: [&str; 6] = ["Right", "Left", "Up", "Down", "Clockwise", "Counter-CW"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RgbSettings {
    pub(crate) effect_idx: usize,
    pub(crate) color_idx: usize,
    pub(crate) brightness: u8,
    pub(crate) speed: u8,
    pub(crate) direction_idx: usize,
}

impl RgbSettings {
    pub(crate) fn from_config(config: &RgbConfig) -> Self {
        Self {
            effect_idx: config.effect.min(RGB_EFFECTS.len() - 1),
            color_idx: config.color.min(COLOR_PALETTE.len() - 1),
            brightness: config.brightness.min(100),
            speed: config.speed.min(100),
            direction_idx: config.direction.min(DIRECTIONS.len() - 1),
        }
    }

    pub(crate) fn to_config(self) -> RgbConfig {
        RgbConfig {
            effect: self.effect_idx,
            color: self.color_idx,
            brightness: self.brightness,
            speed: self.speed,
            direction: self.direction_idx,
        }
    }

    pub(crate) fn effect(&self) -> RgbEffect {
        RGB_EFFECTS[self.effect_idx]
    }

    pub(crate) fn color(&self) -> ColorDef {
        COLOR_PALETTE[self.color_idx]
    }

    /// Jumps a list-valued field straight to option `index`; other fields are left alone.
    pub(crate) fn set(&mut self, field: RgbField, index: usize) {
        match field {
            RgbField::Effect if index < RGB_EFFECTS.len() => self.effect_idx = index,
            RgbField::Color if index < COLOR_PALETTE.len() => self.color_idx = index,
            RgbField::Direction if index < DIRECTIONS.len() => self.direction_idx = index,
            _ => {}
        }
    }

    pub(crate) fn adjust(&mut self, field: RgbField, step: i8) {
        match field {
            RgbField::Effect => {
                self.effect_idx = wrap_index(self.effect_idx, RGB_EFFECTS.len(), step);
            }
            RgbField::Color => {
                self.color_idx = wrap_index(self.color_idx, COLOR_PALETTE.len(), step);
            }
            RgbField::Brightness => {
                self.brightness = adjust_percent(self.brightness, step);
            }
            RgbField::Speed => {
                self.speed = adjust_percent(self.speed, step);
            }
            RgbField::Direction => {
                self.direction_idx = wrap_index(self.direction_idx, DIRECTIONS.len(), step);
            }
        }
    }
}

fn wrap_index(current: usize, len: usize, step: i8) -> usize {
    if len == 0 {
        return 0;
    }

    if step < 0 {
        current.checked_sub(1).unwrap_or(len - 1)
    } else {
        (current + 1) % len
    }
}

fn adjust_percent(current: u8, step: i8) -> u8 {
    let delta = if step < 0 { -5 } else { 5 };
    (current as i16 + delta).clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_config_values_are_clamped() {
        let config = RgbConfig {
            effect: 99,
            color: 99,
            brightness: 140,
            speed: 120,
            direction: 99,
        };

        let rgb = RgbSettings::from_config(&config);

        assert_eq!(rgb.effect_idx, RGB_EFFECTS.len() - 1);
        assert_eq!(rgb.color_idx, COLOR_PALETTE.len() - 1);
        assert_eq!(rgb.brightness, 100);
        assert_eq!(rgb.speed, 100);
        assert_eq!(rgb.direction_idx, DIRECTIONS.len() - 1);
    }

    #[test]
    fn rgb_adjustment_wraps_and_clamps() {
        let mut rgb = RgbSettings::from_config(&RgbConfig::default());

        rgb.effect_idx = 0;
        rgb.adjust(RgbField::Effect, -1);
        assert_eq!(rgb.effect_idx, RGB_EFFECTS.len() - 1);

        rgb.brightness = 95;
        rgb.adjust(RgbField::Brightness, 1);
        assert_eq!(rgb.brightness, 100);

        rgb.speed = 5;
        rgb.adjust(RgbField::Speed, -1);
        assert_eq!(rgb.speed, 0);
    }
}
