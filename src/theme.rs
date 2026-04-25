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
    pub(crate) const ELEVATED: Option<Color> = None;
    
    // -------------------------------------------------------------------------
    // Borders
    // -------------------------------------------------------------------------
    /// Active/Selected panel border (Bright indigo/blue)
    pub(crate) const BORDER: Color = Color::Rgb(122, 162, 247);
    /// Inactive panel border (Deep, subtle slate-blue)
    pub(crate) const BORDER_MUTED: Color = Color::Rgb(41, 46, 66);
    
    // -------------------------------------------------------------------------
    // Typography
    // -------------------------------------------------------------------------
    /// Primary text (Crisp, cool off-white for high readability without eye strain)
    pub(crate) const TEXT: Color = Color::Rgb(192, 202, 245);
    /// Secondary text (Dimmed, used for labels and units like "°C" or "RPM")
    pub(crate) const MUTED: Color = Color::Rgb(86, 95, 137);
    /// Barely visible text (e.g., disabled items or grid lines)
    pub(crate) const SUBTLE: Color = Color::Rgb(59, 66, 97);
    
    // -------------------------------------------------------------------------
    // Accents & Branding
    // -------------------------------------------------------------------------
    /// Main application accent (Vibrant Cyan - perfect for headers/active tabs)
    pub(crate) const ACCENT: Color = Color::Rgb(125, 207, 255);
    /// Secondary accent (Vibrant Purple - good for toggles or secondary states)
    pub(crate) const ACCENT_2: Color = Color::Rgb(187, 154, 247);
    
    // -------------------------------------------------------------------------
    // Semantic & Status Colors
    // -------------------------------------------------------------------------
    pub(crate) const SUCCESS: Color = Color::Rgb(158, 206, 106); // Neon Green
    pub(crate) const WARNING: Color = Color::Rgb(224, 175, 104); // Vibrant Amber
    pub(crate) const DANGER: Color = Color::Rgb(247, 118, 142);  // Punchy Coral/Red
    
    // -------------------------------------------------------------------------
    // Sensor Specific Colors
    // -------------------------------------------------------------------------
    pub(crate) const COOL: Color = Color::Rgb(125, 207, 255);    // Cyan
    pub(crate) const NORMAL: Color = Color::Rgb(158, 206, 106);  // Green
    pub(crate) const WARM: Color = Color::Rgb(255, 158, 100);    // Orange
    pub(crate) const HOT: Color = Color::Rgb(255, 85, 85);       // Intense Red

    /// Determine color based on temperature thresholds
    pub(crate) fn temp_color(value: f64) -> Color {
        // Expanded to 4 tiers for better visual feedback in graphs
        if value < 50.0 {
            Self::COOL     // Idle / Browsing
        } else if value < 75.0 {
            Self::NORMAL   // Moderate load
        } else if value < 85.0 {
            Self::WARM     // Heavy load / Gaming
        } else {
            Self::HOT      // Throttling territory
        }
    }

    /// Determine color based on fan RPM percentage
    pub(crate) fn fan_rpm_color(value: f64, max_rpm: f64) -> Color {
        if value <= 0.0 || max_rpm <= 0.0 {
            Self::MUTED
        } else {
            let ratio = value / max_rpm;
            if ratio < 0.35 {
                Self::COOL     // Quiet mode
            } else if ratio < 0.65 {
                Self::NORMAL   // Audible but fine
            } else if ratio < 0.85 {
                Self::WARNING  // Getting loud
            } else {
                Self::DANGER   // Jet engine
            }
        }
    }
}