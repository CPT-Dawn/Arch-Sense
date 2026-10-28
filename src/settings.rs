//! The settings list: every control the dashboard offers, grouped into sections.
//!
//! [`SettingsModel`] holds what the hardware last reported and turns it into the
//! rows the list shows ([`SettingsModel::rows`]): only the Capabilities the laptop
//! has, and only the lighting controls the current Lighting Effect uses. A key
//! press on a row becomes a [`Change`] for the app to send to the hardware worker.

use std::collections::{BTreeMap, HashMap};

use crate::capabilities::{Capability, Laptop};
use crate::models::{
    COLOR_PALETTE, DIRECTIONS, OFF_EFFECT_INDEX, RANDOM_COLOR_INDEX, RGB_EFFECTS, Rgb, RgbField,
    RgbSettings,
};

/// A group of settings under one heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    Performance,
    Battery,
    System,
    Lighting,
}

impl Section {
    pub(crate) const ALL: [Self; 4] = [
        Self::Performance,
        Self::Battery,
        Self::System,
        Self::Lighting,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Performance => "Performance",
            Self::Battery => "Battery",
            Self::System => "System",
            Self::Lighting => "Lighting",
        }
    }
}

/// One row of the settings list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SettingId {
    /// A Capability of the laptop, read from and written to its attribute file.
    Hardware(Capability),
    /// One field of the keyboard lighting.
    Lighting(RgbField),
}

/// Every setting, in list order.
const ORDER: [SettingId; 13] = [
    SettingId::Hardware(Capability::ThermalProfile),
    SettingId::Hardware(Capability::Fans),
    SettingId::Hardware(Capability::ChargeLimit),
    SettingId::Hardware(Capability::UsbCharging),
    SettingId::Hardware(Capability::BatteryCalibration),
    SettingId::Hardware(Capability::BootAnimation),
    SettingId::Hardware(Capability::LcdOverdrive),
    SettingId::Hardware(Capability::BacklightTimeout),
    SettingId::Lighting(RgbField::Effect),
    SettingId::Lighting(RgbField::Color),
    SettingId::Lighting(RgbField::Brightness),
    SettingId::Lighting(RgbField::Speed),
    SettingId::Lighting(RgbField::Direction),
];

impl SettingId {
    pub(crate) fn section(self) -> Section {
        match self {
            Self::Hardware(Capability::ThermalProfile | Capability::Fans) => Section::Performance,
            Self::Hardware(
                Capability::ChargeLimit | Capability::UsbCharging | Capability::BatteryCalibration,
            ) => Section::Battery,
            Self::Hardware(_) => Section::System,
            Self::Lighting(_) => Section::Lighting,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Hardware(capability) => match capability {
                Capability::ThermalProfile => "Thermal Profile",
                Capability::Fans => "Fans",
                Capability::ChargeLimit => "Charge limit",
                Capability::UsbCharging => "USB charging",
                Capability::BatteryCalibration => "Calibrate battery",
                Capability::BootAnimation => "Boot animation",
                Capability::LcdOverdrive => "LCD overdrive",
                Capability::BacklightTimeout => "Backlight timeout",
            },
            Self::Lighting(field) => match field {
                RgbField::Effect => "Effect",
                RgbField::Color => "Colour",
                RgbField::Brightness => "Brightness",
                RgbField::Speed => "Speed",
                RgbField::Direction => "Direction",
            },
        }
    }

    /// One line explaining the setting, shown on wide terminals.
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Self::Hardware(capability) => match capability {
                Capability::ThermalProfile => "Firmware fan curve and power limits",
                Capability::Fans => "Auto follows the Thermal Profile",
                Capability::ChargeLimit => "Stop charging at 80% to extend battery life",
                Capability::UsbCharging => "Power USB ports while off, down to this %",
                Capability::BatteryCalibration => "Full discharge cycle, keep AC connected",
                Capability::BootAnimation => "Logo and sound at power on",
                Capability::LcdOverdrive => "Less ghosting, slightly more overshoot",
                Capability::BacklightTimeout => "Keyboard light off after 30 s idle",
            },
            Self::Lighting(field) => match field {
                RgbField::Effect => "Rendered by the keyboard itself",
                RgbField::Color => "Colour the effect uses",
                RgbField::Brightness | RgbField::Speed | RgbField::Direction => "",
            },
        }
    }
}

/// How a setting's value is shown and changed inline.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Control {
    /// A segmented choice between `options`; `current` is `None` when the
    /// hardware reports a value none of them match.
    Choice {
        options: Vec<String>,
        current: Option<usize>,
    },
    /// An on/off switch.
    Switch(bool),
    /// A button; `running` while the action it started is still going.
    Action { running: bool },
    /// A percentage.
    Slider(u8),
    /// A palette of colour swatches; `None` stands for Random.
    Swatches {
        colours: Vec<Option<Rgb>>,
        current: usize,
        name: String,
    },
}

impl Control {
    /// The value as words, e.g. "Performance", "On" or "30%".
    pub(crate) fn value_text(&self) -> String {
        match self {
            Self::Choice { options, current } => current
                .and_then(|index| options.get(index))
                .cloned()
                .unwrap_or_else(|| "Custom".to_string()),
            Self::Switch(on) => if *on { "On" } else { "Off" }.to_string(),
            Self::Action { running } => if *running { "Running" } else { "Idle" }.to_string(),
            Self::Slider(percent) => format!("{percent}%"),
            Self::Swatches { name, .. } => name.clone(),
        }
    }
}

/// One row of the settings list, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Setting {
    pub(crate) id: SettingId,
    pub(crate) control: Control,
    /// Why the last change to this setting failed, until a change succeeds.
    pub(crate) error: Option<String>,
}

/// What a key press on a setting asks for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Change {
    /// Write `value` to `capability`'s attribute.
    Write {
        capability: Capability,
        value: String,
    },
    /// Send this lighting to the keyboard.
    Lighting(RgbSettings),
    /// Ask before starting battery calibration.
    ConfirmCalibration,
}

/// What the hardware last reported, plus changes the app has sent since.
#[derive(Clone, Debug, Default)]
pub(crate) struct SettingsModel {
    /// What the last scan detected; `None` until the first scan lands.
    pub(crate) laptop: Option<Laptop>,
    /// The raw value of every Capability present.
    pub(crate) values: BTreeMap<Capability, String>,
    /// The keyboard lighting, when there is a keyboard to light.
    pub(crate) lighting: Option<RgbSettings>,
    /// The last failure per setting.
    pub(crate) errors: HashMap<SettingId, String>,
}

impl SettingsModel {
    /// The visible rows, in list order.
    pub(crate) fn rows(&self) -> Vec<Setting> {
        ORDER
            .into_iter()
            .filter_map(|id| {
                self.control(id).map(|control| Setting {
                    id,
                    control,
                    error: self.errors.get(&id).cloned(),
                })
            })
            .collect()
    }

    /// The control for `id`, or `None` when it isn't shown.
    pub(crate) fn control(&self, id: SettingId) -> Option<Control> {
        match id {
            SettingId::Hardware(capability) => {
                let laptop = self.laptop.as_ref().filter(|l| l.has(capability))?;
                let raw = self.values.get(&capability).map_or("", String::as_str);
                Some(hardware_control(capability, raw, laptop))
            }
            SettingId::Lighting(field) => lighting_control(field, self.lighting.as_ref()?),
        }
    }

    /// The change ←/→ asks for: `step` is -1 or +1.
    pub(crate) fn change(&self, id: SettingId, step: i8) -> Option<Change> {
        match id {
            SettingId::Hardware(capability) => {
                let laptop = self.laptop.as_ref()?;
                match self.control(id)? {
                    Control::Choice { options, current } => {
                        let next = step_index(current, options.len(), step)?;
                        let (value, _) = hardware_choices(capability, laptop).swap_remove(next);
                        Some(Change::Write { capability, value })
                    }
                    Control::Switch(on) => Some(Change::Write {
                        capability,
                        value: if on { "0" } else { "1" }.to_string(),
                    }),
                    Control::Action { .. } => self.activate(id),
                    Control::Slider(_) | Control::Swatches { .. } => None,
                }
            }
            SettingId::Lighting(field) => {
                self.control(id)?;
                let mut lighting = self.lighting?;
                lighting.adjust(field, step);
                Some(Change::Lighting(lighting))
            }
        }
    }

    /// The change Enter/Space asks for: toggle, run the action, or step forward.
    pub(crate) fn activate(&self, id: SettingId) -> Option<Change> {
        match self.control(id)? {
            Control::Action { running: false } => Some(Change::ConfirmCalibration),
            Control::Action { running: true } => Some(Change::Write {
                capability: Capability::BatteryCalibration,
                value: "0".to_string(),
            }),
            _ => self.change(id, 1),
        }
    }

    /// Records `change` as if it had already reached the hardware.
    pub(crate) fn apply(&mut self, change: &Change) {
        match change {
            Change::Write { capability, value } => {
                self.values.insert(*capability, value.clone());
            }
            Change::Lighting(lighting) => self.lighting = Some(*lighting),
            Change::ConfirmCalibration => {}
        }
    }

    /// `value` of `capability` in words, e.g. "Max" for Fans `100,100`.
    pub(crate) fn describe(&self, capability: Capability, value: &str) -> String {
        match self.laptop.as_ref() {
            Some(laptop) => hardware_control(capability, value, laptop).value_text(),
            None => value.to_string(),
        }
    }
}

fn hardware_control(capability: Capability, raw: &str, laptop: &Laptop) -> Control {
    match capability {
        Capability::ThermalProfile | Capability::Fans | Capability::UsbCharging => {
            let choices = hardware_choices(capability, laptop);
            let current = if capability == Capability::Fans {
                fan_choice(raw)
            } else {
                choices.iter().position(|(value, _)| value == raw)
            };
            Control::Choice {
                options: choices.into_iter().map(|(_, label)| label).collect(),
                current,
            }
        }
        Capability::BatteryCalibration => Control::Action {
            running: raw == "1",
        },
        Capability::ChargeLimit
        | Capability::BootAnimation
        | Capability::LcdOverdrive
        | Capability::BacklightTimeout => Control::Switch(raw == "1"),
    }
}

/// The values a choice Capability accepts, each with its label.
fn hardware_choices(capability: Capability, laptop: &Laptop) -> Vec<(String, String)> {
    let pairs = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(value, label)| (value.to_string(), label.to_string()))
            .collect()
    };
    match capability {
        Capability::ThermalProfile => laptop
            .thermal_profiles
            .iter()
            .map(|profile| (profile.clone(), thermal_label(profile)))
            .collect(),
        // Auto hands the fans back to the firmware; Max pins them. Nothing in between.
        Capability::Fans => pairs(&[("0,0", "Auto"), ("100,100", "Max")]),
        Capability::UsbCharging => {
            pairs(&[("0", "Off"), ("10", "10%"), ("20", "20%"), ("30", "30%")])
        }
        _ => Vec::new(),
    }
}

/// Auto when every fan is at 0, Max when every fan is at 100 or more.
fn fan_choice(raw: &str) -> Option<usize> {
    let speeds: Option<Vec<u32>> = raw
        .split(',')
        .map(|part| part.trim().parse().ok())
        .collect();
    let speeds = speeds.filter(|speeds| !speeds.is_empty())?;
    if speeds.iter().all(|&speed| speed == 0) {
        Some(0)
    } else if speeds.iter().all(|&speed| speed >= 100) {
        Some(1)
    } else {
        None
    }
}

/// A Thermal Profile name as people say it, e.g. `balanced-performance` → "Balanced Performance".
fn thermal_label(profile: &str) -> String {
    profile
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn lighting_control(field: RgbField, lighting: &RgbSettings) -> Option<Control> {
    let effect = lighting.effect();
    let off = lighting.effect_idx == OFF_EFFECT_INDEX;
    let names = |names: &mut dyn Iterator<Item = &str>| names.map(str::to_owned).collect();
    match field {
        RgbField::Effect => Some(Control::Choice {
            options: names(&mut RGB_EFFECTS.iter().map(|effect| effect.name)),
            current: Some(lighting.effect_idx),
        }),
        RgbField::Color if effect.has_color => Some(Control::Swatches {
            colours: COLOR_PALETTE
                .iter()
                .enumerate()
                .map(|(index, colour)| (index != RANDOM_COLOR_INDEX).then_some(colour.rgb))
                .collect(),
            current: lighting.color_idx,
            name: lighting.color().name.to_string(),
        }),
        RgbField::Brightness if !off => Some(Control::Slider(lighting.brightness)),
        RgbField::Speed if !off && effect.name != "Static" => Some(Control::Slider(lighting.speed)),
        RgbField::Direction if effect.has_direction => Some(Control::Choice {
            options: names(&mut DIRECTIONS.iter().copied()),
            current: Some(lighting.direction_idx),
        }),
        _ => None,
    }
}

/// The index `step` away from `current` in a list of `len`, wrapping around.
fn step_index(current: Option<usize>, len: usize, step: i8) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match current {
        None if step < 0 => len - 1,
        None => 0,
        Some(index) if step < 0 => index.checked_sub(1).unwrap_or(len - 1),
        Some(index) => (index + 1) % len,
    })
}
