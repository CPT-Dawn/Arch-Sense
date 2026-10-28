//! The settings list: every control the dashboard offers, grouped into sections.
//!
//! [`SettingsModel`] holds what the hardware last reported and turns it into the
//! rows the list shows ([`SettingsModel::rows`]): only the Capabilities the laptop
//! has, and only the lighting controls the current Lighting Effect uses. A key
//! press on a row becomes a [`Change`] for the app to send to the hardware worker.

use std::collections::{BTreeMap, HashMap};

use crate::capabilities::{Capability, Laptop};
use crate::lighting::{Colour, Lighting, LightingBackend, PALETTE, Rgb};
use crate::models::FanMode;

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
    /// One parameter of the keyboard lighting.
    Lighting(LightingField),
}

/// The lighting parameters the Lighting section can show. Which appear depends
/// on what the current Lighting Effect uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LightingField {
    Effect,
    /// Which zone the Colour row paints: All, or one of a multi-zone keyboard's.
    Zone,
    Colour,
    Brightness,
    Speed,
    Direction,
}

impl LightingField {
    /// Every lighting parameter, in list order.
    const ALL: [Self; 6] = [
        Self::Effect,
        Self::Zone,
        Self::Colour,
        Self::Brightness,
        Self::Speed,
        Self::Direction,
    ];
}

/// Every setting, in list order: the Capabilities in [`Capability::ALL`]'s
/// order, then the lighting parameters.
fn order() -> impl Iterator<Item = SettingId> {
    Capability::ALL
        .into_iter()
        .map(SettingId::Hardware)
        .chain(LightingField::ALL.into_iter().map(SettingId::Lighting))
}

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
                LightingField::Effect => "Effect",
                LightingField::Zone => "Zone",
                LightingField::Colour => "Colour",
                LightingField::Brightness => "Brightness",
                LightingField::Speed => "Speed",
                LightingField::Direction => "Direction",
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
                LightingField::Effect => "Rendered by the keyboard itself",
                LightingField::Zone => "Colour one zone, or all at once",
                LightingField::Colour => "←→ palette · ⏎ or # custom hex",
                LightingField::Brightness | LightingField::Speed | LightingField::Direction => "",
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
    /// A palette of colour swatches, then a "# custom" affordance. A `None`
    /// swatch stands for Random; `current` is `None` for a custom colour.
    Swatches {
        colours: Vec<Option<Rgb>>,
        current: Option<usize>,
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
    /// Preview this lighting on the keyboard (once input settles) and save it.
    Lighting(Lighting),
    /// Pick which zone the Colour row paints (0 = All, 1.. = a zone). Touches no hardware.
    Zone(usize),
    /// Ask for a custom hex colour.
    CustomColour,
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
    pub(crate) lighting: Option<Lighting>,
    /// The last failure per setting.
    pub(crate) errors: HashMap<SettingId, String>,
    /// The zone the Colour row paints: 0 for All, otherwise 1-based. UI state,
    /// not part of the [`Lighting`].
    pub(crate) zone: usize,
}

impl SettingsModel {
    /// The visible rows, in list order.
    pub(crate) fn rows(&self) -> Vec<Setting> {
        order()
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
            SettingId::Lighting(field) => {
                lighting_control(field, self.lighting.as_ref()?, self.zone)
            }
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
                let control = self.control(id)?;
                if let (LightingField::Zone, Control::Choice { options, current }) =
                    (field, &control)
                {
                    return Some(Change::Zone(step_index(*current, options.len(), step)?));
                }
                let lighting = self.lighting.as_ref()?;
                let zone = self.painted_zone(lighting);
                Some(Change::Lighting(adjust_lighting(
                    lighting, field, &control, step, zone,
                )?))
            }
        }
    }

    /// The change a custom colour asks for: the selected zone, or every zone, takes `rgb`.
    pub(crate) fn custom_colour(&self, rgb: Rgb) -> Option<Change> {
        let lighting = self.lighting.as_ref()?;
        let mut next = lighting.clone();
        paint(&mut next, self.painted_zone(lighting), Colour::Rgb(rgb));
        Some(Change::Lighting(next))
    }

    /// The zone (0-based) a colour change paints, or `None` for every zone: only
    /// an effect that colours zones separately honours the Zone selection.
    fn painted_zone(&self, lighting: &Lighting) -> Option<usize> {
        let zone = self.zone.checked_sub(1)?;
        (lighting.effect.uses.zones && zone < lighting.colours.len()).then_some(zone)
    }

    /// The change a click on option `index` of `id` asks for: set that option
    /// directly, or toggle/run a switch or button.
    pub(crate) fn choose(&self, id: SettingId, index: usize) -> Option<Change> {
        match id {
            SettingId::Hardware(capability) => {
                let laptop = self.laptop.as_ref()?;
                match self.control(id)? {
                    Control::Choice { options, .. } if index < options.len() => {
                        let (value, _) = hardware_choices(capability, laptop).swap_remove(index);
                        Some(Change::Write { capability, value })
                    }
                    Control::Switch(_) | Control::Action { .. } => self.activate(id),
                    _ => None,
                }
            }
            SettingId::Lighting(field) => {
                let lighting = self.lighting.as_ref()?;
                let backend = lighting.backend;
                let mut next = lighting.clone();
                match (field, self.control(id)?) {
                    (LightingField::Effect, _) => next.effect = backend.effects().get(index)?,
                    (LightingField::Zone, Control::Choice { options, .. }) => {
                        return (index < options.len()).then_some(Change::Zone(index));
                    }
                    (LightingField::Direction, _) => {
                        next.direction = backend.directions().get(index)?;
                    }
                    // The option just past the swatches is "# custom".
                    (LightingField::Colour, Control::Swatches { colours, .. }) => {
                        match colours.get(index) {
                            Some(&swatch) => paint(
                                &mut next,
                                self.painted_zone(lighting),
                                swatch_colour(swatch),
                            ),
                            None if index == colours.len() => return Some(Change::CustomColour),
                            None => return None,
                        }
                    }
                    _ => return None,
                }
                Some(Change::Lighting(next))
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
            Control::Swatches { .. } => Some(Change::CustomColour),
            _ => self.change(id, 1),
        }
    }

    /// Records `change` as if it had already reached the hardware.
    pub(crate) fn apply(&mut self, change: &Change) {
        match change {
            Change::Write { capability, value } => {
                self.values.insert(*capability, value.clone());
            }
            Change::Lighting(lighting) => self.lighting = Some(lighting.clone()),
            Change::Zone(zone) => self.zone = *zone,
            Change::CustomColour | Change::ConfirmCalibration => {}
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
                FanMode::from_fan_speed(raw)
                    .and_then(|mode| FanMode::ALL.iter().position(|&m| m == mode))
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
            .map(|profile| (profile.name().to_string(), profile.label()))
            .collect(),
        Capability::Fans => FanMode::ALL
            .iter()
            .map(|mode| (mode.fan_speed().to_string(), mode.label().to_string()))
            .collect(),
        Capability::UsbCharging => {
            pairs(&[("0", "Off"), ("10", "10%"), ("20", "20%"), ("30", "30%")])
        }
        _ => Vec::new(),
    }
}

/// A lighting row's control, or `None` when the current effect doesn't use it.
fn lighting_control(field: LightingField, lighting: &Lighting, zone: usize) -> Option<Control> {
    let backend = lighting.backend;
    let uses = lighting.effect.uses;
    match field {
        LightingField::Effect => Some(Control::Choice {
            options: backend
                .effects()
                .iter()
                .map(|e| e.name.to_string())
                .collect(),
            current: backend.effects().iter().position(|e| e == lighting.effect),
        }),
        LightingField::Zone if uses.colour && uses.zones && backend.zones() > 1 => {
            let options = ["All".to_string()]
                .into_iter()
                .chain((1..=backend.zones()).map(|zone| zone.to_string()))
                .collect();
            Some(Control::Choice {
                options,
                current: Some(zone.min(backend.zones())),
            })
        }
        LightingField::Colour if uses.colour => {
            let colours = swatches(backend);
            let selected = zone.checked_sub(1).filter(|_| uses.zones);
            let shown = match selected {
                Some(zone) => lighting.colours.get(zone).copied(),
                None if uses.zones && !all_alike(&lighting.colours) => None,
                None => Some(lighting.colour()),
            };
            Some(Control::Swatches {
                current: shown.and_then(|colour| {
                    colours
                        .iter()
                        .position(|&swatch| swatch_colour(swatch) == colour)
                }),
                colours,
                name: shown.map_or_else(|| "Mixed".to_string(), Colour::name),
            })
        }
        LightingField::Brightness if uses.brightness => Some(Control::Slider(lighting.brightness)),
        LightingField::Speed if uses.speed => Some(Control::Slider(lighting.speed)),
        LightingField::Direction if uses.direction => Some(Control::Choice {
            options: backend.directions().iter().map(|d| d.to_string()).collect(),
            current: backend
                .directions()
                .iter()
                .position(|&d| d == lighting.direction),
        }),
        _ => None,
    }
}

/// The swatches the Colour row offers: the palette, then Random where the backend has it.
fn swatches(backend: LightingBackend) -> Vec<Option<Rgb>> {
    let palette = PALETTE.iter().map(|&(_, rgb)| Some(rgb));
    let random = backend.has_random_colour().then_some(None);
    palette.chain(random).collect()
}

fn swatch_colour(swatch: Option<Rgb>) -> Colour {
    swatch.map_or(Colour::Random, Colour::Rgb)
}

/// `lighting` with `field` stepped by `step`: ±1 option, or ±5% for a slider.
fn adjust_lighting(
    lighting: &Lighting,
    field: LightingField,
    control: &Control,
    step: i8,
    zone: Option<usize>,
) -> Option<Lighting> {
    let mut next = lighting.clone();
    let backend = lighting.backend;
    let percent = |value: u8| (value as i16 + if step < 0 { -5 } else { 5 }).clamp(0, 100) as u8;
    match (field, control) {
        (LightingField::Effect, Control::Choice { options, current }) => {
            next.effect = &backend.effects()[step_index(*current, options.len(), step)?];
        }
        (LightingField::Direction, Control::Choice { options, current }) => {
            next.direction = backend.directions()[step_index(*current, options.len(), step)?];
        }
        (
            LightingField::Colour,
            Control::Swatches {
                colours, current, ..
            },
        ) => {
            let swatch = colours[step_index(*current, colours.len(), step)?];
            paint(&mut next, zone, swatch_colour(swatch));
        }
        (LightingField::Brightness, _) => next.brightness = percent(lighting.brightness),
        (LightingField::Speed, _) => next.speed = percent(lighting.speed),
        _ => return None,
    }
    Some(next)
}

/// Gives `zone` (0-based), or every zone when `None`, `colour`.
fn paint(lighting: &mut Lighting, zone: Option<usize>, colour: Colour) {
    match zone.and_then(|zone| lighting.colours.get_mut(zone)) {
        Some(slot) => *slot = colour,
        None => lighting.colours.fill(colour),
    }
}

fn all_alike(colours: &[Colour]) -> bool {
    colours.windows(2).all(|pair| pair[0] == pair[1])
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
