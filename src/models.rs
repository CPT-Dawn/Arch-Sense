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
