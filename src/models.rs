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
