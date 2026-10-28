//! The panel shown where the keyboard preview would be when the laptop's
//! keyboard is one Arch-Sense recognises but can't drive yet.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::lighting::{CONTRIBUTING_URL, UnsupportedKeyboard};
use crate::theme::Theme;

/// Rows the panel takes, rule included.
pub(super) const HEIGHT: u16 = 5;

pub(super) fn draw(frame: &mut Frame, area: Rect, keyboard: &UnsupportedKeyboard) {
    let block = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(Theme::LINE))
        .title(Span::styled(
            " LIGHTING ",
            Style::new().fg(Theme::DIM).bold(),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(Span::styled(
            "This keyboard isn't supported yet",
            Style::new().fg(Theme::WARN).bold(),
        )),
        Line::from(Span::styled(
            format!("Your {keyboard} is not one Arch-Sense can light up, so it is left alone."),
            Style::new().fg(Theme::SUB),
        )),
        Line::from(Span::styled(
            "You can help: share USB captures of the vendor app changing colours. How:",
            Style::new().fg(Theme::SUB),
        )),
        Line::from(Span::styled(
            CONTRIBUTING_URL,
            Style::new().fg(Theme::ACCENT_ALT).underlined(),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner.inner(Margin::new(2, 0)));
}
