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
    /// Both modes, in the order the Fans row offers them.
    pub(crate) const ALL: [Self; 2] = [Self::Auto, Self::Max];

    /// What Linuwu-Sense's `fan_speed` takes for this mode: 0 hands every fan
    /// back to the firmware, 100 pins it. Arch-Sense offers nothing in between.
    pub(crate) fn fan_speed(self) -> &'static str {
        match self {
            Self::Auto => "0,0",
            Self::Max => "100,100",
        }
    }

    /// The mode a `fan_speed` value means: Auto when every fan is at 0, Max when
    /// every fan is at 100 or more, `None` for a custom speed set elsewhere.
    pub(crate) fn from_fan_speed(raw: &str) -> Option<Self> {
        let speeds: Option<Vec<u32>> = raw
            .split(',')
            .map(|part| part.trim().parse().ok())
            .collect();
        let speeds = speeds.filter(|speeds| !speeds.is_empty())?;
        if speeds.iter().all(|&speed| speed == 0) {
            Some(Self::Auto)
        } else if speeds.iter().all(|&speed| speed >= 100) {
            Some(Self::Max)
        } else {
            None
        }
    }

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
