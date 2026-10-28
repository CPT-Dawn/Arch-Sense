//! Draws the dashboard (layout C, "Settings list"), top to bottom:
//!
//! - [`header`]: brand, laptop model, Sense Interface, Lighting Backend;
//! - [`cards`]: CPU, GPU, Fans and Battery gauge cards;
//! - [`list`]: the scrollable settings list, grouped by section;
//! - a pinned area below the list (empty for now; the keyboard preview goes here);
//! - [`footer`]: status on the left, key hints on the right;
//! - [`overlay`]: any modal, drawn over everything.
//!
//! Rendering only reads the [`App`]; the one thing it records is where clickable
//! things landed ([`App::record_hit`]) and the list's scroll position.

mod cards;
mod footer;
mod header;
mod list;
mod overlay;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::theme::Theme;

/// The smallest terminal the dashboard lays out in.
pub(crate) const MIN_WIDTH: u16 = 80;
pub(crate) const MIN_HEIGHT: u16 = 24;
/// From this width up, rows show their hint and the footer shows every key.
pub(crate) const WIDE: u16 = 110;
const CARDS_HEIGHT: u16 = 5;

pub(crate) fn draw(frame: &mut Frame, app: &App) {
    app.hits.borrow_mut().clear();
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        too_small(frame, area);
        return;
    }

    let [head, cards, list, pinned, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(CARDS_HEIGHT),
        Constraint::Fill(1),
        Constraint::Length(pinned_height(app)),
        Constraint::Length(1),
    ])
    .areas(area);

    header::draw(frame, head, app);
    cards::draw(frame, cards, app);
    list::draw(frame, list, app);
    draw_pinned(frame, pinned, app);
    footer::draw(frame, foot, app);
    overlay::draw(frame, area, app);
}

/// Rows reserved below the list for a pinned panel, such as the keyboard
/// preview while a Lighting row is selected. None yet.
fn pinned_height(_app: &App) -> u16 {
    0
}

fn draw_pinned(_frame: &mut Frame, _area: Rect, _app: &App) {}

fn too_small(frame: &mut Frame, area: Rect) {
    let message = vec![
        Line::from(Span::styled(
            "◆ ARCH-SENSE",
            Style::new().fg(Theme::ACCENT).bold(),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            format!(
                "Terminal is {}×{}, I need at least {MIN_WIDTH}×{MIN_HEIGHT}.",
                area.width, area.height
            ),
            Style::new().fg(Theme::SUB),
        )),
        Line::from(Span::styled(
            "Make me bigger, or press q to quit.",
            Style::new().fg(Theme::DIM),
        )),
    ];
    let height = message.len() as u16;
    let top = area.y + area.height.saturating_sub(height) / 2;
    let centred = Rect {
        y: top,
        height: height.min(area.height),
        ..area
    };
    frame.render_widget(
        Paragraph::new(message).alignment(Alignment::Center),
        centred,
    );
}
