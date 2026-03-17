use ratatui::style::Color;

pub(crate) struct Theme;

impl Theme {
    pub(crate) const ACCENT: Color = Color::Rgb(57, 255, 20);
    pub(crate) const ACCENT2: Color = Color::Rgb(0, 200, 60);
    pub(crate) const DIM: Color = Color::Rgb(0, 140, 40);
    pub(crate) const DARK: Color = Color::Rgb(0, 60, 20);
    pub(crate) const BG_HL: Color = Color::Rgb(10, 40, 15);
    pub(crate) const BG_HEADER: Color = Color::Rgb(5, 20, 8);
    pub(crate) const FG: Color = Color::Rgb(210, 225, 210);
    pub(crate) const FG_DIM: Color = Color::Rgb(100, 130, 100);
    pub(crate) const COOL: Color = Color::Rgb(57, 255, 20);
    pub(crate) const WARM: Color = Color::Rgb(255, 200, 0);
    pub(crate) const HOT: Color = Color::Rgb(255, 50, 30);
    pub(crate) const ERR: Color = Color::Rgb(255, 70, 50);

    pub(crate) fn temp_color(c: f64) -> Color {
        if c < 55.0 {
            Self::COOL
        } else if c < 78.0 {
            Self::WARM
        } else {
            Self::HOT
        }
    }

    pub(crate) fn fan_color(p: u32) -> Color {
        if p == 0 {
            Self::FG_DIM
        } else if p < 50 {
            Self::COOL
        } else if p < 80 {
            Self::WARM
        } else {
            Self::HOT
        }
    }
}
