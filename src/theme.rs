use ratatui::style::Color;

pub(crate) struct Theme;

impl Theme {
    // -------------------------------------------------------------------------
    // Backgrounds & Surfaces
    // We leave these None to respect the user's terminal transparency,
    // but you can define them if you use filled blocks.
    // -------------------------------------------------------------------------
    pub(crate) const BG: Option<Color> = None;
    pub(crate) const SURFACE: Option<Color> = None;
    pub(crate) const ELEVATED: Option<Color> = Some(Color::Rgb(28, 36, 51));

    // -------------------------------------------------------------------------
    // Borders
    // -------------------------------------------------------------------------
    /// Global frame border for header/footer framing.
    pub(crate) const BORDER_FRAME: Color = Color::Rgb(83, 112, 153);
    /// Focused panel border (electric cyan/blue).
    pub(crate) const BORDER_FOCUS: Color = Color::Rgb(99, 183, 255);
    /// Inactive panel border (subtle slate).
    pub(crate) const BORDER_IDLE: Color = Color::Rgb(58, 69, 96);

    // -------------------------------------------------------------------------
    // Typography
    // -------------------------------------------------------------------------
    /// High-emphasis readable text.
    pub(crate) const TEXT_PRIMARY: Color = Color::Rgb(220, 228, 244);
    /// Supporting labels and less important values.
    pub(crate) const TEXT_SECONDARY: Color = Color::Rgb(154, 169, 198);
    /// Tertiary hints, separators, and passive metadata.
    pub(crate) const TEXT_TERTIARY: Color = Color::Rgb(116, 129, 157);
    /// Disabled and de-emphasized text.
    pub(crate) const TEXT_DISABLED: Color = Color::Rgb(88, 99, 124);

    // -------------------------------------------------------------------------
    // Accents & Branding
    // -------------------------------------------------------------------------
    /// Main brand/accent color for key highlights.
    pub(crate) const BRAND_PRIMARY: Color = Color::Rgb(95, 182, 255);
    /// Tertiary accent for animated pulse blending.
    pub(crate) const BRAND_TERTIARY: Color = Color::Rgb(160, 149, 245);

    // -------------------------------------------------------------------------
    // Semantic & Status Colors
    // -------------------------------------------------------------------------
    pub(crate) const STATE_INFO: Color = Color::Rgb(106, 189, 255);
    pub(crate) const STATE_SUCCESS: Color = Color::Rgb(128, 214, 145);
    pub(crate) const STATE_WARNING: Color = Color::Rgb(243, 189, 101);
    pub(crate) const STATE_ERROR: Color = Color::Rgb(240, 111, 132);

    // -------------------------------------------------------------------------
    // Sensor Specific Colors
    // -------------------------------------------------------------------------
    pub(crate) const TEMP_COOL: Color = Color::Rgb(112, 196, 255);
    pub(crate) const TEMP_NORMAL: Color = Color::Rgb(128, 214, 145);
    pub(crate) const TEMP_WARM: Color = Color::Rgb(242, 186, 101);
    pub(crate) const TEMP_HOT: Color = Color::Rgb(240, 106, 123);

    pub(crate) const FAN_QUIET: Color = Color::Rgb(120, 201, 255);
    pub(crate) const FAN_NORMAL: Color = Color::Rgb(131, 208, 153);
    pub(crate) const FAN_LOUD: Color = Color::Rgb(239, 180, 94);
    pub(crate) const FAN_MAX: Color = Color::Rgb(236, 101, 119);

    /// Interactive/value emphasis colors.
    pub(crate) const VALUE_PRIMARY: Color = Color::Rgb(96, 186, 255);
    pub(crate) const VALUE_SELECTED: Color = Color::Rgb(95, 225, 214);

    /// Determine color based on temperature thresholds
    pub(crate) fn temp_color(value: f64) -> Color {
        if value < 50.0 {
            Self::TEMP_COOL
        } else if value < 75.0 {
            Self::TEMP_NORMAL
        } else if value < 85.0 {
            Self::TEMP_WARM
        } else {
            Self::TEMP_HOT
        }
    }

    /// Determine color based on fan RPM percentage
    pub(crate) fn fan_rpm_color(value: f64, max_rpm: f64) -> Color {
        if value <= 0.0 || max_rpm <= 0.0 {
            Self::TEXT_TERTIARY
        } else {
            let ratio = value / max_rpm;
            if ratio < 0.35 {
                Self::FAN_QUIET
            } else if ratio < 0.65 {
                Self::FAN_NORMAL
            } else if ratio < 0.85 {
                Self::FAN_LOUD
            } else {
                Self::FAN_MAX
            }
        }
    }
}
