use crate::config::RgbConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusPanel {
    Controls,
    Rgb,
    Sensors,
}

impl FocusPanel {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Controls => Self::Rgb,
            Self::Rgb => Self::Controls, // Skip Sensors - it's read-only
            Self::Sensors => Self::Controls,
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Controls => Self::Rgb,
            Self::Rgb => Self::Controls, // Skip Sensors - it's read-only
            Self::Sensors => Self::Rgb,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ControlId {
    ThermalProfile,
    BacklightTimeout,
    BatteryCalibration,
    BatteryLimiter,
    BootAnimation,
    FanSpeed,
    LcdOverride,
    UsbCharging,
}

impl ControlId {
    pub(crate) const ALL: [Self; 8] = [
        Self::ThermalProfile,
        Self::BatteryLimiter,
        Self::FanSpeed,
        Self::BacklightTimeout,
        Self::BatteryCalibration,
        Self::BootAnimation,
        Self::LcdOverride,
        Self::UsbCharging,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ThermalProfile => "Thermal Profile",
            Self::BacklightTimeout => "Backlight Timeout",
            Self::BatteryCalibration => "Battery Calibration",
            Self::BatteryLimiter => "Battery Limiter",
            Self::BootAnimation => "Boot Animation",
            Self::FanSpeed => "Fan Speed",
            Self::LcdOverride => "LCD Override",
            Self::UsbCharging => "USB Charging",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ControlChoice {
    pub(crate) value: String,
    pub(crate) label: String,
}

impl ControlChoice {
    pub(crate) fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ControlKind {
    Toggle,
    Choice(Vec<ControlChoice>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ControlItem {
    pub(crate) id: ControlId,
    pub(crate) raw: String,
    pub(crate) display: String,
    pub(crate) kind: ControlKind,
    pub(crate) pending: Option<usize>,
    pub(crate) last_error: Option<String>,
}

impl ControlItem {
    pub(crate) fn label(&self) -> &'static str {
        self.id.label()
    }

    pub(crate) fn pending_choice(&self) -> Option<&ControlChoice> {
        match (&self.kind, self.pending) {
            (ControlKind::Choice(choices), Some(index)) => choices.get(index),
            _ => None,
        }
    }

    pub(crate) fn visible_value(&self) -> String {
        self.pending_choice()
            .map(|choice| choice.label.clone())
            .unwrap_or_else(|| self.display.clone())
    }

    pub(crate) fn current_choice_index(&self) -> Option<usize> {
        match &self.kind {
            ControlKind::Choice(choices) => choices
                .iter()
                .position(|choice| choice.value == self.raw)
                .or(Some(0)),
            ControlKind::Toggle => None,
        }
    }
}

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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SensorSnapshot {
    pub(crate) cpu_temp: SensorMetric,
    pub(crate) gpu_temp: SensorMetric,
    pub(crate) cpu_fan: SensorMetric,
    pub(crate) gpu_fan: SensorMetric,
    pub(crate) cpu_fan_mode: FanMode,
    pub(crate) gpu_fan_mode: FanMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RgbField {
    Effect,
    Color,
    Brightness,
    Speed,
    Direction,
}

impl RgbField {
    pub(crate) const ALL: [Self; 5] = [
        Self::Effect,
        Self::Color,
        Self::Brightness,
        Self::Speed,
        Self::Direction,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Effect => "Mode",
            Self::Color => "Color",
            Self::Brightness => "Brightness",
            Self::Speed => "Speed",
            Self::Direction => "Direction",
        }
    }
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

    pub(crate) fn direction_name(&self) -> &'static str {
        DIRECTIONS[self.direction_idx]
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
    let delta = if step < 0 { -10 } else { 10 };
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
