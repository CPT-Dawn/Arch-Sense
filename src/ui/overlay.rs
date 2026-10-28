//! Modals drawn over the dashboard. [`modal`] clears and frames a centred box;
//! each overlay fills it.

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph};

use crate::app::{App, Overlay};
use crate::lighting::Rgb;
use crate::theme::Theme;

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    match &app.overlay {
        None => {}
        Some(Overlay::ConfirmCalibration) => confirm_calibration(frame, area),
        Some(Overlay::Hex(digits)) => custom_colour(frame, area, digits),
    }
}

/// Clears a `width` × `height` box in the middle of `area`, frames it with
/// `title` and returns the space inside.
pub(super) fn modal(frame: &mut Frame, area: Rect, width: u16, height: u16, title: &str) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let boxed = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, boxed);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::ACCENT))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title.to_string(), Style::new().fg(Theme::ACCENT).bold()),
            Span::raw(" "),
        ]))
        .padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(boxed);
    frame.render_widget(block, boxed);
    inner
}

fn confirm_calibration(frame: &mut Frame, area: Rect) {
    let inner = modal(frame, area, 58, 8, "Calibrate battery?");
    let lines = vec![
        Line::from(Span::styled(
            "Drains to 0% and recharges fully. Takes a few hours.",
            Style::new().fg(Theme::SUB),
        )),
        Line::from(Span::styled(
            "Keep AC connected the whole time.",
            Style::new().fg(Theme::WARN),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                " y ",
                Style::new().fg(Theme::ON_ACCENT).bg(Theme::ACCENT).bold(),
            ),
            Span::styled(" start    ", Style::new().fg(Theme::SUB)),
            Span::styled(" n ", Style::new().fg(Theme::TEXT).bg(Theme::LINE).bold()),
            Span::styled(" cancel", Style::new().fg(Theme::SUB)),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The custom-colour input: the digits typed so far and, once there are six,
/// a swatch of the colour they make.
fn custom_colour(frame: &mut Frame, area: Rect, digits: &str) {
    let inner = modal(frame, area, 40, 7, "Custom colour");
    let colour = Rgb::from_hex(digits);
    let swatch = match colour {
        Some(rgb) => Span::styled("██████", Style::new().fg(Color::Rgb(rgb.r, rgb.g, rgb.b))),
        None => Span::styled("░░░░░░", Style::new().fg(Theme::LINE)),
    };
    let help = if colour.is_some() {
        "⏎ apply · esc cancel"
    } else {
        "type 6 hex digits · esc cancel"
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("# ", Style::new().fg(Theme::DIM)),
            Span::styled(format!("{digits:_<6}"), Style::new().fg(Theme::TEXT).bold()),
            Span::raw("   "),
            swatch,
        ]),
        Line::raw(""),
        Line::from(Span::styled(help, Style::new().fg(Theme::DIM))),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
