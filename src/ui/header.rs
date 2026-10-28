//! The header line: brand, laptop model, Sense Interface and Lighting Backend.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::capabilities::{SenseInterface, Turbo};
use crate::keyboard::UsbAccess;
use crate::theme::Theme;

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let mut details = Vec::new();
    if let Some(model) = &app.model {
        details.push(model.clone());
    }
    match &app.settings.laptop {
        Some(laptop) => {
            details.push(sense_interface(laptop.sense_interface).to_string());
            details.push(lighting_backend(&app.keyboard).to_string());
        }
        None => details.push("Scanning hardware…".to_string()),
    }

    let brand = Line::from(vec![
        Span::styled(" ◆ ", Style::new().fg(Theme::ACCENT)),
        Span::styled("ARCH-SENSE", Style::new().fg(Theme::TEXT).bold()),
        Span::styled(
            format!("  {}", details.join(" · ")),
            Style::new().fg(Theme::DIM),
        ),
    ]);
    frame.render_widget(Paragraph::new(brand), area);

    let turbo = app.settings.laptop.as_ref().and_then(|laptop| laptop.turbo);
    if let Some(turbo) = turbo {
        let badge = match turbo {
            Turbo::Engaged => Span::styled("TURBO ", Style::new().fg(Theme::WARN).bold()),
            Turbo::Off => Span::styled("Turbo off ", Style::new().fg(Theme::DIM)),
        };
        frame.render_widget(Paragraph::new(badge).alignment(Alignment::Right), area);
    }
}

fn sense_interface(interface: SenseInterface) -> &'static str {
    match interface {
        SenseInterface::Predator => "Predator Sense",
        SenseInterface::Nitro => "Nitro Sense",
        SenseInterface::None => "No Sense Interface",
    }
}

fn lighting_backend(keyboard: &UsbAccess) -> &'static str {
    match keyboard {
        UsbAccess::Accessible => "USB HID keyboard",
        UsbAccess::PermissionDenied => "USB HID keyboard (no access)",
        UsbAccess::Error(_) => "USB HID keyboard (unreachable)",
        UsbAccess::NotFound => "No keyboard lighting",
    }
}
