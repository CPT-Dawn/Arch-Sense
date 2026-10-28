//! Modals drawn over the dashboard. [`modal`] clears and frames a centred box;
//! each overlay fills it.

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph};

use crate::app::{App, Overlay};
use crate::theme::Theme;

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    match &app.overlay {
        None => {}
        Some(Overlay::ConfirmCalibration) => confirm_calibration(frame, area),
        Some(Overlay::Help) => help(frame, area),
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

const BINDINGS: &[(&str, &str)] = &[
    ("↑ k  ↓ j", "Select the previous or next setting"),
    ("← h  → l", "Change the selected setting"),
    ("Enter  Space", "Toggle, run, or step forward"),
    ("Tab  Shift-Tab", "Jump between sections"),
    ("?", "Show or hide this help"),
    ("d", "Open the Doctor"),
    ("Esc", "Close any dialog"),
    ("q  Ctrl-C", "Quit"),
    ("click", "Select a row; click an option to set it"),
    ("click selected", "Step the selected row forward"),
    ("right-click", "Step the selected row back"),
    ("scroll", "Move the selection"),
];

fn help(frame: &mut Frame, area: Rect) {
    let height = BINDINGS.len() as u16 + 4;
    let inner = modal(frame, area, 66, height, "Help");
    let mut lines: Vec<Line> = BINDINGS
        .iter()
        .map(|(keys, what)| {
            Line::from(vec![
                Span::styled(format!("{keys:<16}"), Style::new().fg(Theme::ACCENT).bold()),
                Span::styled(*what, Style::new().fg(Theme::SUB)),
            ])
        })
        .collect();
    lines.push(Line::from(Span::styled(
        "Esc to close",
        Style::new().fg(Theme::DIM),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}
