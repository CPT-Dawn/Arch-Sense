//! The footer: status message on the left, key hints on the right.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::WIDE;
use crate::app::{App, MessageLevel};
use crate::theme::Theme;

const ALL_KEYS: &[(&str, &str)] = &[
    ("↑↓", "select"),
    ("←→", "change"),
    ("⏎", "toggle"),
    ("tab", "section"),
    ("?", "help"),
    ("d", "doctor"),
    ("q", "quit"),
];
const FEW_KEYS: &[(&str, &str)] = &[
    ("←→", "change"),
    ("?", "help"),
    ("d", "doctor"),
    ("q", "quit"),
];

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let keys = if area.width >= WIDE {
        ALL_KEYS
    } else {
        FEW_KEYS
    };
    let hints: Vec<Span> = keys
        .iter()
        .flat_map(|(key, action)| {
            [
                Span::styled(format!(" {key} "), Style::new().fg(Theme::ACCENT).bold()),
                Span::styled(format!("{action} "), Style::new().fg(Theme::DIM)),
            ]
        })
        .collect();
    let hints_width = hints.iter().map(Span::width).sum::<usize>() as u16;
    let [left, right] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(hints_width)]).areas(area);

    frame.render_widget(Paragraph::new(status(app)), left);
    frame.render_widget(Paragraph::new(Line::from(hints)), right);
}

fn status(app: &App) -> Span<'static> {
    if let Some(message) = app.status() {
        let colour = match message.level {
            MessageLevel::Info => Theme::SUB,
            MessageLevel::Success => Theme::OK,
            MessageLevel::Error => Theme::ERROR,
        };
        return Span::styled(format!(" {}", message.text), Style::new().fg(colour));
    }
    if !app.scanned() {
        Span::styled(" Scanning hardware…", Style::new().fg(Theme::DIM))
    } else if !app.module_loaded() {
        Span::styled(
            " Linuwu-Sense isn't loaded. Press d for the Doctor",
            Style::new().fg(Theme::WARN),
        )
    } else {
        Span::styled(" Changes apply instantly", Style::new().fg(Theme::DIM))
    }
}
