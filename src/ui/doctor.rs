//! The Doctor overlay: the same checklist `arch-sense doctor` prints, scrollable.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::overlay::modal;
use crate::doctor::{Report, Status};
use crate::theme::Theme;

const MAX_WIDTH: u16 = 104;

pub(super) fn draw(frame: &mut Frame, area: Rect, report: &Report, scroll: u16) {
    // Borders and modal padding take 6 columns and 3 rows; the key hint takes a row.
    let width = area.width.saturating_sub(4).min(MAX_WIDTH);
    let lines = lines(report, usize::from(width.saturating_sub(6)));
    let wanted = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let height = wanted.min(area.height.saturating_sub(2));
    let inner = modal(frame, area, width, height, "Doctor");

    let [body, hint] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);
    let overflow = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_sub(body.height);
    let scroll = scroll.min(overflow);
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), body);

    let mut keys = vec![
        Span::styled(" esc ", Style::new().fg(Theme::ACCENT).bold()),
        Span::styled("close ", Style::new().fg(Theme::DIM)),
    ];
    if overflow > 0 {
        keys.extend([
            Span::styled(" ↑↓ ", Style::new().fg(Theme::ACCENT).bold()),
            Span::styled("scroll ", Style::new().fg(Theme::DIM)),
        ]);
    }
    keys.push(Span::styled(
        "  same as `arch-sense doctor`",
        Style::new().fg(Theme::DIM),
    ));
    frame.render_widget(Paragraph::new(Line::from(keys)), hint);
}

/// One line per check (wrapped to `width`), its remedy indented underneath.
fn lines(report: &Report, width: usize) -> Vec<Line<'static>> {
    let name_width = report
        .checks()
        .iter()
        .map(|check| check.name.chars().count())
        .max()
        .unwrap_or(0);
    let indent = 2 + name_width + 2;
    let text_width = width.saturating_sub(indent).max(10);

    let mut lines = Vec::new();
    for check in report.checks() {
        let colour = match check.status {
            Status::Ok => Theme::OK,
            Status::Info => Theme::ACCENT,
            Status::Warn => Theme::WARN,
            Status::Error => Theme::ERROR,
        };
        for (index, chunk) in wrap(&check.detail, text_width).into_iter().enumerate() {
            let lead = if index == 0 {
                vec![
                    Span::styled(
                        format!("{} ", check.status.mark()),
                        Style::new().fg(colour).bold(),
                    ),
                    Span::styled(
                        format!("{:name_width$}  ", check.name),
                        Style::new().fg(Theme::TEXT).bold(),
                    ),
                ]
            } else {
                vec![Span::raw(" ".repeat(indent))]
            };
            let mut spans = lead;
            spans.push(Span::styled(chunk, Style::new().fg(Theme::SUB)));
            lines.push(Line::from(spans));
        }
        if let Some(remedy) = &check.remedy {
            for (index, chunk) in wrap(remedy, text_width.saturating_sub(2).max(10))
                .into_iter()
                .enumerate()
            {
                let arrow = if index == 0 { "→ " } else { "  " };
                lines.push(Line::from(vec![
                    Span::raw(" ".repeat(indent)),
                    Span::styled(arrow, Style::new().fg(colour)),
                    Span::styled(chunk, Style::new().fg(Theme::TEXT)),
                ]));
            }
        }
    }
    lines
}

/// Splits `text` into lines of at most `width` characters, at spaces where it can.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let fits = line.chars().count() + 1 + word.chars().count() <= width;
        if !line.is_empty() && !fits {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
        while line.chars().count() > width {
            let rest: String = line.chars().skip(width).collect();
            line = line.chars().take(width).collect();
            lines.push(std::mem::replace(&mut line, rest));
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}
