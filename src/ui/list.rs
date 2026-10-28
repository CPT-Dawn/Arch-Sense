//! The settings list: one full-width, scrollable list grouped by section, with
//! every value shown inline as a segmented choice, switch, slider, swatches or
//! button.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

use super::WIDE;
use crate::app::{App, Hit};
use crate::lighting::Rgb;
use crate::settings::{Control, Section, Setting, SettingId};
use crate::theme::Theme;

/// Columns for the selection marker plus the label.
const LABEL_WIDTH: u16 = 20;
/// Columns for the right-aligned hint on wide terminals.
const HINT_WIDTH: u16 = 44;
/// The longest a slider bar gets.
const SLIDER_CELLS: usize = 24;

/// One line of the list.
enum Entry {
    Heading(Section),
    Row(Setting),
    Gap,
}

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let inner = area.inner(Margin::new(1, 1));
    if inner.height == 0 {
        return;
    }

    let rows = app.rows();
    if rows.is_empty() {
        let text = if !app.scanned() {
            "Scanning hardware…"
        } else if !app.module_loaded() {
            "No controls: Linuwu-Sense isn't loaded. Press d for the Doctor."
        } else {
            "This laptop exposes no controls."
        };
        frame.render_widget(
            Paragraph::new(Span::styled(text, Style::new().fg(Theme::DIM)))
                .alignment(Alignment::Center),
            Rect {
                y: inner.y + inner.height / 2,
                height: 1,
                ..inner
            },
        );
        return;
    }

    let selected = app.selected_id();
    let entries = entries(rows);
    let selected_line = entries
        .iter()
        .position(|entry| matches!(entry, Entry::Row(row) if Some(row.id) == selected));

    let height = inner.height as usize;
    let offset = scroll_offset(app.scroll.get(), &entries, selected_line, height);
    app.scroll.set(offset);

    let wide = area.width >= WIDE;
    for (line, entry) in entries.iter().skip(offset).take(height).enumerate() {
        let line_area = Rect {
            y: inner.y + line as u16,
            height: 1,
            ..inner
        };
        match entry {
            Entry::Heading(section) => frame.render_widget(
                Paragraph::new(heading(section.title(), inner.width)),
                line_area,
            ),
            Entry::Gap => {}
            Entry::Row(row) => draw_row(frame, line_area, app, row, Some(row.id) == selected, wide),
        }
    }

    let more = Style::new().fg(Theme::DIM);
    let marker_x = inner.right().saturating_sub(1);
    if offset > 0 {
        frame.render_widget(Span::styled("▲", more), Rect::new(marker_x, area.y, 1, 1));
    }
    if offset + height < entries.len() {
        let y = area.bottom().saturating_sub(1);
        frame.render_widget(Span::styled("▼", more), Rect::new(marker_x, y, 1, 1));
    }
}

/// Section headings, their rows, and a blank line between sections.
fn entries(rows: Vec<Setting>) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut current = None;
    for row in rows {
        let section = row.id.section();
        if current != Some(section) {
            if current.is_some() {
                entries.push(Entry::Gap);
            }
            entries.push(Entry::Heading(section));
            current = Some(section);
        }
        entries.push(Entry::Row(row));
    }
    entries
}

/// The first line to show so the selection stays in view, moving as little as
/// possible from `previous`. A selection at the top of its section brings the
/// heading along.
fn scroll_offset(
    previous: usize,
    entries: &[Entry],
    selected: Option<usize>,
    height: usize,
) -> usize {
    let mut offset = previous.min(entries.len().saturating_sub(height));
    if let Some(selected) = selected {
        let top = match selected.checked_sub(1).map(|above| &entries[above]) {
            Some(Entry::Heading(_)) => selected - 1,
            _ => selected,
        };
        if top < offset {
            offset = top;
        }
        if selected >= offset + height {
            offset = selected + 1 - height;
        }
    }
    offset
}

fn heading(title: &str, width: u16) -> Line<'static> {
    let title = format!(" {} ", title.to_uppercase());
    let rule = "─".repeat((width as usize).saturating_sub(title.chars().count() + 1));
    Line::from(vec![
        Span::styled(title, Style::new().fg(Theme::DIM).bold()),
        Span::styled(rule, Style::new().fg(Theme::LINE)),
    ])
}

fn draw_row(frame: &mut Frame, area: Rect, app: &App, row: &Setting, selected: bool, wide: bool) {
    app.record_hit(area, Hit::Row(row.id));
    if selected {
        frame.render_widget(
            Block::new().style(Style::new().bg(Theme::SELECTED_BG)),
            area,
        );
    }

    let label = Line::from(vec![
        Span::styled(
            if selected { "▌ " } else { "  " },
            Style::new().fg(Theme::ACCENT),
        ),
        Span::styled(
            row.id.label(),
            Style::new().fg(if selected { Theme::TEXT } else { Theme::SUB }),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(label),
        Rect {
            width: LABEL_WIDTH.min(area.width),
            ..area
        },
    );

    let hint_width = if wide { HINT_WIDTH } else { 0 };
    let control_area = Rect {
        x: area.x + LABEL_WIDTH,
        width: area.width.saturating_sub(LABEL_WIDTH + hint_width),
        ..area
    };
    let control = inline_control(app, row, selected, control_area);
    let used = control.width() as u16;
    frame.render_widget(Paragraph::new(control), control_area);

    // An error takes the hint's place, or whatever room the control leaves.
    let note_area = if wide {
        Rect {
            x: area.right() - hint_width,
            width: hint_width,
            ..area
        }
    } else {
        let x = control_area.x + used + 1;
        Rect {
            x,
            width: area.right().saturating_sub(x),
            ..area
        }
    };
    let note = match &row.error {
        Some(reason) => Some(error_note(reason, note_area.width)),
        None if wide => Some(Span::styled(row.id.hint(), Style::new().fg(Theme::DIM))),
        None => None,
    };
    if let Some(note) = note {
        frame.render_widget(Paragraph::new(note).alignment(Alignment::Right), note_area);
    }
}

/// The longest error wording that fits in `width`, always pointing at the Doctor if it can.
fn error_note(reason: &str, width: u16) -> Span<'static> {
    let text = [
        format!("✗ {reason} · press d for the Doctor"),
        format!("✗ {reason} · d Doctor"),
        format!("✗ {reason}"),
    ]
    .into_iter()
    .find(|text| text.chars().count() <= width as usize)
    .unwrap_or_else(|| format!("✗ {reason}"));
    Span::styled(text, Style::new().fg(Theme::ERROR))
}

/// Builds a row's inline control, recording each clickable option as it goes.
struct Builder<'a> {
    app: &'a App,
    id: SettingId,
    x: u16,
    y: u16,
    spans: Vec<Span<'static>>,
}

impl Builder<'_> {
    fn push(&mut self, span: Span<'static>, option: Option<usize>) {
        let width = span.width() as u16;
        if let Some(option) = option {
            self.app.record_hit(
                Rect::new(self.x, self.y, width, 1),
                Hit::Option(self.id, option),
            );
        }
        self.x += width;
        self.spans.push(span);
    }
}

fn inline_control(app: &App, row: &Setting, selected: bool, area: Rect) -> Line<'static> {
    let mut out = Builder {
        app,
        id: row.id,
        x: area.x,
        y: area.y,
        spans: Vec::new(),
    };
    let room = area.width as usize;

    match &row.control {
        Control::Choice { options, current } => {
            let widths: Vec<usize> = options
                .iter()
                .map(|option| option.chars().count() + 3)
                .collect();
            let (from, to) = window(&widths, current.unwrap_or(0), room);
            if from > 0 {
                out.push(Span::styled("… ", Style::new().fg(Theme::DIM)), None);
            }
            for (index, option) in options.iter().enumerate().take(to).skip(from) {
                let style = if Some(index) == *current {
                    let fill = if selected { Theme::ACCENT } else { Theme::SUB };
                    Style::new().fg(Theme::ON_ACCENT).bg(fill).bold()
                } else {
                    Style::new().fg(Theme::SUB)
                };
                out.push(Span::styled(format!(" {option} "), style), Some(index));
                out.push(Span::raw(" "), None);
            }
            if to < options.len() {
                out.push(Span::styled("…", Style::new().fg(Theme::DIM)), None);
            }
        }
        Control::Switch(on) => {
            let (knob, colour) = if *on {
                ("━━━●", Theme::OK)
            } else {
                ("○━━━", Theme::DIM)
            };
            out.push(Span::styled(knob, Style::new().fg(colour)), Some(0));
            let (text, colour) = if *on {
                ("  On", Theme::TEXT)
            } else {
                ("  Off", Theme::DIM)
            };
            out.push(Span::styled(text, Style::new().fg(colour)), None);
        }
        Control::Slider(percent) => {
            let cells = SLIDER_CELLS.min(room.saturating_sub(8)).max(4);
            let filled = *percent as usize * cells / 100;
            let fill = if selected { Theme::ACCENT } else { Theme::SUB };
            out.push(
                Span::styled("━".repeat(filled), Style::new().fg(fill)),
                None,
            );
            out.push(Span::styled("●", Style::new().fg(Theme::TEXT)), None);
            out.push(
                Span::styled("─".repeat(cells - filled), Style::new().fg(Theme::LINE)),
                None,
            );
            out.push(
                Span::styled(format!("  {percent}%"), Style::new().fg(Theme::TEXT)),
                None,
            );
        }
        Control::Swatches {
            colours,
            current,
            name,
        } => {
            for (index, colour) in colours.iter().enumerate() {
                let (glyph, colour) = match colour {
                    Some(rgb) => ("█", to_color(*rgb)),
                    None => ("?", Theme::ACCENT_ALT),
                };
                let swatch = if Some(index) == *current {
                    format!("▐{glyph}▌")
                } else {
                    format!(" {glyph} ")
                };
                out.push(Span::styled(swatch, Style::new().fg(colour)), Some(index));
            }
            out.push(
                Span::styled(format!("  {name}"), Style::new().fg(Theme::TEXT)),
                None,
            );
            out.push(Span::raw("  "), None);
            // The option just past the swatches is "type a custom colour".
            out.push(
                Span::styled("# custom", Style::new().fg(Theme::DIM)),
                Some(colours.len()),
            );
        }
        Control::Action { running } => {
            if *running {
                out.push(
                    Span::styled("Running  ", Style::new().fg(Theme::WARN)),
                    None,
                );
                out.push(
                    Span::styled("[ Stop ]", Style::new().fg(Theme::WARN).bold()),
                    Some(0),
                );
            } else {
                out.push(
                    Span::styled(
                        "[ Start calibration ]",
                        Style::new().fg(Theme::ACCENT_ALT).bold(),
                    ),
                    Some(0),
                );
            }
        }
    }
    Line::from(out.spans)
}

/// The options `from..to` that fit in `room` columns, grown outwards from
/// `current`. When not all fit, two columns each side stay free for ellipses.
fn window(widths: &[usize], current: usize, room: usize) -> (usize, usize) {
    if widths.iter().sum::<usize>() <= room || widths.is_empty() {
        return (0, widths.len());
    }
    let current = current.min(widths.len() - 1);
    let budget = room.saturating_sub(4);
    let (mut from, mut to) = (current, current + 1);
    let mut used = widths[current];
    loop {
        let mut grew = false;
        if to < widths.len() && used + widths[to] <= budget {
            used += widths[to];
            to += 1;
            grew = true;
        }
        if from > 0 && used + widths[from - 1] <= budget {
            from -= 1;
            used += widths[from];
            grew = true;
        }
        if !grew {
            return (from, to);
        }
    }
}

fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}
