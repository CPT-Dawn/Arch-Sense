//! The one hand-tuned palette. The background is left to the terminal, so
//! transparency shows through; only the selected row gets a fill.

use ratatui::style::Color;

pub(crate) struct Theme;

impl Theme {
    /// High-emphasis text: values and the selected label.
    pub(crate) const TEXT: Color = Color::Rgb(226, 232, 240);
    /// Labels and unselected options.
    pub(crate) const SUB: Color = Color::Rgb(148, 163, 184);
    /// Hints, section titles and passive metadata.
    pub(crate) const DIM: Color = Color::Rgb(100, 116, 139);
    /// Borders and rules.
    pub(crate) const LINE: Color = Color::Rgb(51, 65, 85);
    /// The selected row's fill.
    pub(crate) const SELECTED_BG: Color = Color::Rgb(30, 41, 59);
    /// Text drawn on an accent fill, such as the current segment.
    pub(crate) const ON_ACCENT: Color = Color::Black;

    pub(crate) const ACCENT: Color = Color::Rgb(34, 211, 238);
    pub(crate) const ACCENT_ALT: Color = Color::Rgb(167, 139, 250);

    pub(crate) const OK: Color = Color::Rgb(74, 222, 128);
    pub(crate) const WARN: Color = Color::Rgb(251, 191, 36);
    pub(crate) const ERROR: Color = Color::Rgb(248, 113, 113);

    /// Cool to hot, for temperature readings.
    pub(crate) fn temperature(celsius: f64) -> Color {
        match celsius {
            c if c < 55.0 => Self::ACCENT,
            c if c < 70.0 => Self::OK,
            c if c < 85.0 => Self::WARN,
            _ => Self::ERROR,
        }
    }
}
