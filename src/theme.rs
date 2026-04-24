use ratatui::style::Color;

pub(crate) struct Theme;

impl Theme {
    /// Use terminal's default background (supports transparency/blur).
    /// Widgets should not force background colors unless specifically needed for contrast.
    pub(crate) const BG: Option<Color> = None;
    pub(crate) const SURFACE: Option<Color> = None;
    pub(crate) const SURFACE_ALT: Option<Color> = None;
    pub(crate) const ELEVATED: Option<Color> = None;
    
    /// Border colors
    pub(crate) const BORDER: Color = Color::Rgb(71, 85, 105);
    pub(crate) const BORDER_MUTED: Color = Color::Rgb(51, 65, 85);
    
    /// Text colors
    pub(crate) const TEXT: Color = Color::Rgb(226, 232, 240);
    pub(crate) const MUTED: Color = Color::Rgb(148, 163, 184);
    pub(crate) const SUBTLE: Color = Color::Rgb(100, 116, 139);
    
    /// Accent and status colors
    pub(crate) const ACCENT: Color = Color::Rgb(56, 189, 248);
    pub(crate) const ACCENT_2: Color = Color::Rgb(167, 139, 250);
    pub(crate) const SUCCESS: Color = Color::Rgb(52, 211, 153);
    pub(crate) const WARNING: Color = Color::Rgb(251, 191, 36);
    pub(crate) const DANGER: Color = Color::Rgb(248, 113, 113);
    pub(crate) const COOL: Color = Color::Rgb(96, 165, 250);
    pub(crate) const WARM: Color = Color::Rgb(251, 146, 60);
    pub(crate) const HOT: Color = Color::Rgb(239, 68, 68);

    pub(crate) fn temp_color(value: f64) -> Color {
        if value < 55.0 {
            Self::COOL
        } else if value < 78.0 {
            Self::WARM
        } else {
            Self::HOT
        }
    }

    pub(crate) fn fan_color(value: f64) -> Color {
        if value == 0.0 {
            Self::MUTED
        } else if value < 50.0 {
            Self::SUCCESS
        } else if value < 80.0 {
            Self::WARNING
        } else {
            Self::DANGER
        }
    }
}
