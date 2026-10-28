//! The one hand-tuned palette. The background is left to the terminal, so
//! transparency shows through; only the selected row gets a fill.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

/// How many colours the terminal gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// The hand-tuned RGB palette.
    Truecolor,
    /// The 16 named ANSI colours.
    Ansi16,
    /// No colour at all (`NO_COLOR`).
    None,
}

impl ColorMode {
    /// Picks a mode from the environment, read through `var` so tests can
    /// supply their own. A non-empty `NO_COLOR` wins; `COLORTERM` of
    /// `truecolor` or `24bit` allows RGB; anything else gets 16 colours.
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Self {
        if var("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            return Self::None;
        }
        match var("COLORTERM").as_deref() {
            Some("truecolor" | "24bit") => Self::Truecolor,
            _ => Self::Ansi16,
        }
    }

    /// Rewrites every colour in `buffer` for this mode.
    pub(crate) fn apply(self, buffer: &mut Buffer) {
        if self == Self::Truecolor {
            return;
        }
        for cell in &mut buffer.content {
            match self {
                Self::Truecolor => {}
                Self::Ansi16 => {
                    cell.fg = to_ansi16(cell.fg);
                    cell.bg = to_ansi16(cell.bg);
                }
                Self::None => {
                    // Without colour, a filled accent (the current option) is
                    // told apart by inverting it instead.
                    let filled = [Theme::ACCENT, Theme::ACCENT_ALT, Theme::SUB].contains(&cell.bg);
                    cell.fg = Color::Reset;
                    cell.bg = Color::Reset;
                    if filled {
                        cell.modifier.insert(Modifier::REVERSED);
                    }
                }
            }
        }
    }
}

/// Maps a palette colour to a named colour: exact for the theme, nearest otherwise.
fn to_ansi16(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let themed = [
        (Theme::TEXT, Color::White),
        (Theme::SUB, Color::Gray),
        (Theme::DIM, Color::DarkGray),
        (Theme::LINE, Color::DarkGray),
        (Theme::SELECTED_BG, Color::Reset),
        (Theme::ACCENT, Color::Cyan),
        (Theme::ACCENT_ALT, Color::Magenta),
        (Theme::OK, Color::Green),
        (Theme::WARN, Color::Yellow),
        (Theme::ERROR, Color::Red),
    ];
    if let Some((_, named)) = themed.iter().find(|(rgb, _)| *rgb == color) {
        return *named;
    }
    const NAMED: [(Color, [i32; 3]); 16] = [
        (Color::Black, [0, 0, 0]),
        (Color::Red, [205, 0, 0]),
        (Color::Green, [0, 205, 0]),
        (Color::Yellow, [205, 205, 0]),
        (Color::Blue, [0, 0, 238]),
        (Color::Magenta, [205, 0, 205]),
        (Color::Cyan, [0, 205, 205]),
        (Color::Gray, [229, 229, 229]),
        (Color::DarkGray, [127, 127, 127]),
        (Color::LightRed, [255, 0, 0]),
        (Color::LightGreen, [0, 255, 0]),
        (Color::LightYellow, [255, 255, 0]),
        (Color::LightBlue, [92, 92, 255]),
        (Color::LightMagenta, [255, 0, 255]),
        (Color::LightCyan, [0, 255, 255]),
        (Color::White, [255, 255, 255]),
    ];
    let distance = |[nr, ng, nb]: [i32; 3]| {
        (nr - i32::from(r)).pow(2) + (ng - i32::from(g)).pow(2) + (nb - i32::from(b)).pow(2)
    };
    NAMED
        .iter()
        .min_by_key(|(_, rgb)| distance(*rgb))
        .map_or(Color::Reset, |(named, _)| *named)
}

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
