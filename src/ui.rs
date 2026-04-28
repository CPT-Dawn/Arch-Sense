use std::collections::VecDeque;

use ratatui::prelude::*;
use ratatui::symbols;
use ratatui::widgets::*;

use crate::app::{AnimatedMetric, App, MessageLevel};
use crate::models::{FanMode, FocusPanel, Rgb, RgbField, COLOR_PALETTE, RANDOM_COLOR_INDEX};
use crate::permissions::UsbAccess;
use crate::theme::Theme;

/// Consistent spacing/padding throughout the UI (in character units)
const SPACING: u16 = 1;

const DOUBLE_SQUIRCLE_BORDER: symbols::border::Set<'static> = symbols::border::Set {
    top_left: symbols::line::ROUNDED.top_left,
    top_right: symbols::line::ROUNDED.top_right,
    bottom_left: symbols::line::ROUNDED.bottom_left,
    bottom_right: symbols::line::ROUNDED.bottom_right,
    vertical_left: symbols::line::ROUNDED.vertical,
    vertical_right: symbols::line::ROUNDED.vertical,
    horizontal_top: symbols::line::ROUNDED.horizontal,
    horizontal_bottom: symbols::line::ROUNDED.horizontal,
};

pub(crate) fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Don't force a background color - let the terminal's default show through
    let base_style = match Theme::BG {
        Some(bg) => Style::new().bg(bg),
        None => Style::new(),
    };
    frame.render_widget(Block::new().style(base_style), area);

    // Standardized vertical layout: Header (5), Body (Min 0), Footer (5)
    // We reduce the vertical margin to 0 to let the lines hit the edges if desired, 
    // but keep horizontal margin for breathing room.
    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .horizontal_margin(SPACING)
    .areas(area);

    draw_header(frame, header_area);
    draw_body(frame, body_area, app);
    draw_footer(frame, footer_area, app);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App) {
    // 50/50 split for better visual balance on modern screens
    let [left, right] = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .spacing(SPACING)
    .areas(area);

    // Even split for controls and keyboard
    let [controls, rgb] = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .spacing(SPACING)
    .areas(left);

    draw_controls(frame, controls, app);
    draw_rgb(frame, rgb, app);
    draw_sensors(frame, right, app);
}

fn panel_block<'a>(title: &'a str, panel: FocusPanel, app: &App) -> Block<'a> {
    let focused = app.focus == panel;
    let border = if focused {
        pulse_color(app, Theme::BORDER_FOCUS, Theme::BRAND_TERTIARY)
    } else {
        Theme::BORDER_IDLE
    };

    let title_style = Style::new()
        .fg(if focused {
            Theme::TEXT_PRIMARY
        } else {
            Theme::TEXT_SECONDARY
        })
        .bold();

    let icon = match panel {
        FocusPanel::Controls => " ⚙ ",
        FocusPanel::Rgb => " ⌨ ",
        FocusPanel::Sensors => " 📊 ",
    };

    let mut title_spans = vec![
        Span::styled(icon, Style::new().fg(Theme::BRAND_PRIMARY)),
        Span::styled(format!("{title} "), title_style),
    ];

    match panel {
        FocusPanel::Controls => {
            let (label, color) = module_title_status(app.module_loaded);
            title_spans.push(Span::styled(
                format!(" {label} "),
                Style::new().fg(color).bold(),
            ));
        }
        FocusPanel::Rgb => {
            let (label, color) = keyboard_title_status(&app.keyboard);
            title_spans.push(Span::styled(
                format!(" {label} "),
                Style::new().fg(color).bold(),
            ));
        }
        FocusPanel::Sensors => {}
    }

    // Apply background color only if it's Some, otherwise use terminal default
    let mut block = Block::bordered()
        .border_set(DOUBLE_SQUIRCLE_BORDER)
        .border_style(Style::new().fg(border))
        .title(Line::from(title_spans));

    // Apply optional background
    block = match Theme::SURFACE {
        Some(bg) => block.style(Style::new().bg(bg)),
        None => block,
    };

    block
}

fn module_title_status(module_loaded: bool) -> (&'static str, Color) {
    if module_loaded {
        ("Detected ✅", Theme::TEXT_SECONDARY)
    } else {
        ("Kernel Missing ❌", Theme::STATE_ERROR)
    }
}

fn pulse_color(app: &App, base: Color, pulse: Color) -> Color {
    if app.focus_pulse <= 0.01 {
        return base;
    }

    let mix = app.focus_pulse.clamp(0.0, 1.0);
    blend(base, pulse, mix)
}

fn blend(a: Color, b: Color, mix: f64) -> Color {
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return a;
    };

    let channel = |from: u8, to: u8| (from as f64 + (to as f64 - from as f64) * mix).round() as u8;

    Color::Rgb(channel(ar, br), channel(ag, bg), channel(ab, bb))
}

fn draw_header(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_set(symbols::border::DOUBLE)
        .border_style(Style::new().fg(Theme::BORDER_FRAME));

    f.render_widget(block, area);

    let title = Line::from(vec![
        Span::styled(" ◆ ", Style::new().fg(Theme::BRAND_PRIMARY).bold()),
        Span::styled(
            "A R C H - S E N S E",
            Style::new().fg(Theme::BRAND_PRIMARY).bold(),
        ),
        Span::styled(" ◆ ", Style::new().fg(Theme::BRAND_PRIMARY).bold()),
        Span::styled(
            "Acer Predator Control Center",
            Style::new().fg(Theme::TEXT_SECONDARY),
        ),
    ])
    .centered();

    // Vertically center text in the 5-row area (row 2 is middle)
    let title_area = Rect::new(area.x, area.y + 1, area.width, 1);
    f.render_widget(Paragraph::new(title), title_area);
}

fn keyboard_title_status(access: &UsbAccess) -> (&'static str, Color) {
    match access {
        UsbAccess::Accessible => ("Detected ✅", Theme::TEXT_SECONDARY),
        UsbAccess::PermissionDenied => ("Permission Denied 🔒", Theme::STATE_WARNING),
        UsbAccess::NotFound => ("Not Found ⚠️", Theme::STATE_WARNING),
        UsbAccess::Error(_) => ("Error 🚫", Theme::STATE_ERROR),
    }
}

/// Helper function to apply optional background color to a style
fn style_with_bg(base: Style, bg: Option<Color>) -> Style {
    match bg {
        Some(color) => base.bg(color),
        None => base,
    }
}

fn draw_controls(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Controls", FocusPanel::Controls, app);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Apply inner padding/margin to the content area
    let content_area = Layout::vertical([Constraint::Min(0)])
        .margin(SPACING)
        .split(inner)[0];

    if app.controls.is_empty() {
        frame.render_widget(
            Paragraph::new(" Waiting for hardware controls...")
                .style(Style::new().fg(Theme::TEXT_SECONDARY))
                .alignment(Alignment::Center),
            content_area,
        );
        return;
    }

    let rows = app
        .controls
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let selected = app.focus == FocusPanel::Controls && index == app.selected_control;
            let pending = item.pending.is_some();
            let error = item.last_error.is_some();
            
            // Define the row background style
            let row_style = if selected {
                style_with_bg(Style::new(), Theme::ELEVATED)
            } else {
                Style::new()
            };

            let base_style = if selected {
                Style::new().fg(Theme::TEXT_PRIMARY).bold()
            } else {
                Style::new().fg(Theme::TEXT_PRIMARY)
            };
            
            let value_style = if error {
                Style::new().fg(Theme::STATE_ERROR)
            } else if pending {
                Style::new().fg(Theme::STATE_WARNING).bold()
            } else if selected {
                Style::new().fg(Theme::VALUE_SELECTED).bold()
            } else {
                Style::new().fg(Theme::VALUE_PRIMARY)
            };
            
            let marker = if selected { "▸ " } else { "  " };
            let state = if app.control_pending == Some(item.id) {
                "APPLY"
            } else if pending {
                "PREVIEW"
            } else if error {
                "ERROR"
            } else {
                ""
            };

            Row::new(vec![
                Cell::from(marker).style(base_style),
                Cell::from(item.label()).style(base_style),
                Cell::from(item.visible_value()).style(value_style),
                Cell::from(state).style(Style::new().fg(control_state_color(
                    app.control_pending == Some(item.id),
                    pending,
                    error,
                ))),
            ]).style(row_style)
        })
        .collect::<Vec<_>>();

    let widths = [
        Constraint::Length(2),
        Constraint::Percentage(42),
        Constraint::Percentage(38),
        Constraint::Length(8),
    ];

    frame.render_widget(
        Table::new(rows, widths).column_spacing(SPACING),
        content_area,
    );
}

fn draw_rgb(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Keyboard", FocusPanel::Rgb, app);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Apply inner padding/margin to the content area
    let content_area = Layout::vertical([Constraint::Min(0)])
        .margin(SPACING)
        .split(inner)[0];

    let [rows_area, palette_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .spacing(SPACING)
    .areas(content_area);

    draw_rgb_rows(frame, rows_area, app);
    draw_palette(frame, palette_area, app);
}

fn draw_rgb_rows(frame: &mut Frame, area: Rect, app: &App) {
    let effect = app.rgb.effect();
    let fields = [
        (RgbField::Effect, effect.name.to_string()),
        (RgbField::Color, color_value(app)),
        (RgbField::Brightness, format!("{}%", app.rgb.brightness)),
        (RgbField::Speed, format!("{}%", app.rgb.speed)),
        (RgbField::Direction, direction_value(app)),
    ];

    let rows = fields
        .into_iter()
        .enumerate()
        .map(|(index, (field, value))| {
            let selected = app.focus == FocusPanel::Rgb && index == app.selected_rgb_field;
            
            let row_style = if selected {
                style_with_bg(Style::new(), Theme::ELEVATED)
            } else {
                Style::new()
            };

            let style = if selected {
                Style::new().fg(Theme::TEXT_PRIMARY).bold()
            } else {
                Style::new().fg(Theme::TEXT_PRIMARY)
            };
            
            let value_style = if selected {
                Style::new().fg(Theme::VALUE_SELECTED).bold()
            } else {
                Style::new().fg(Theme::VALUE_PRIMARY)
            };

            Row::new(vec![
                Cell::from(if selected { "▸ " } else { "  " }).style(style),
                Cell::from(field.label()).style(style),
                Cell::from(value).style(value_style),
            ]).style(row_style)
        })
        .collect::<Vec<_>>();

    let widths = [
        Constraint::Length(2),
        Constraint::Percentage(40),
        Constraint::Percentage(60),
    ];

    frame.render_widget(Table::new(rows, widths), area);
}

fn color_value(app: &App) -> String {
    if !app.rgb.effect().has_color {
        "Not used".to_string()
    } else {
        app.rgb.color().name.to_string()
    }
}

fn direction_value(app: &App) -> String {
    if app.rgb.effect().has_direction {
        app.rgb.direction_name().to_string()
    } else {
        "Not used".to_string()
    }
}

fn draw_palette(frame: &mut Frame, area: Rect, app: &App) {
    let mut swatches = vec![Span::styled(
        " 🎨 Palette  ",
        Style::new().fg(Theme::TEXT_SECONDARY),
    )];
    for (index, color) in COLOR_PALETTE.iter().enumerate() {
        let selected = index == app.rgb.color_idx;
        let style = if index == RANDOM_COLOR_INDEX {
            Style::new().fg(Theme::BRAND_TERTIARY).bold()
        } else {
            Style::new().fg(to_color(color.rgb)).bold()
        };

        if selected {
            swatches.push(Span::styled("●", style));
        } else {
            swatches.push(Span::styled("○", style));
        }
        swatches.push(Span::raw(" "));
    }

    frame.render_widget(Paragraph::new(Line::from(swatches)).centered(), area);
}

fn draw_sensors(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Sensors", FocusPanel::Sensors, app);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Apply inner padding/margin to the content area
    let content_area = Layout::vertical([Constraint::Min(0)])
        .margin(SPACING)
        .split(inner)[0];

    let [temps_area, fans_area] = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .spacing(SPACING)
    .areas(content_area);

    draw_overlay_chart(
        frame,
        temps_area,
        "Temperatures",
        &app.sensors.cpu_temp,
        &app.sensors.cpu_temp_history,
        &app.sensors.gpu_temp,
        &app.sensors.gpu_temp_history,
        MetricKind::Temp,
        None,
        None,
    );
    draw_overlay_chart(
        frame,
        fans_area,
        "Fan Speeds",
        &app.sensors.cpu_fan,
        &app.sensors.cpu_fan_history,
        &app.sensors.gpu_fan,
        &app.sensors.gpu_fan_history,
        MetricKind::Fan,
        Some(app.sensors.cpu_fan_mode),
        Some(app.sensors.gpu_fan_mode),
    );
}

#[derive(Clone, Copy)]
enum MetricKind {
    Temp,
    Fan,
}

fn draw_overlay_chart(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    cpu_metric: &AnimatedMetric,
    cpu_history: &VecDeque<u64>,
    gpu_metric: &AnimatedMetric,
    gpu_history: &VecDeque<u64>,
    kind: MetricKind,
    cpu_mode: Option<FanMode>,
    gpu_mode: Option<FanMode>,
) {
    if area.height < 5 {
        return;
    }

    let [header_area, chart_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
        .spacing(1) // Add space between header and chart
        .areas(area);

    let cpu_color = if cpu_metric.error.is_some() {
        Theme::TEXT_DISABLED
    } else {
        metric_sample_color(kind, cpu_metric.value, cpu_metric.max)
    };
    let gpu_color = if gpu_metric.error.is_some() {
        Theme::TEXT_DISABLED
    } else {
        metric_sample_color(kind, gpu_metric.value, gpu_metric.max)
    };

    let cpu_val = metric_value(cpu_metric, kind);
    let gpu_val = metric_value(gpu_metric, kind);

    // Header with polished legend
    let mut header_spans = vec![
        Span::styled(format!("{title:<14}"), Style::new().fg(Theme::TEXT_PRIMARY).bold()),
        Span::styled("● ", Style::new().fg(cpu_color)),
        Span::styled("CPU ", Style::new().fg(Theme::TEXT_SECONDARY)),
        Span::styled(format!("{cpu_val} "), Style::new().fg(cpu_color).bold()),
    ];

    if let Some(mode) = cpu_mode {
        header_spans.push(Span::styled(
            format!("[{}] ", mode.label()),
            Style::new().fg(fan_mode_color(mode)),
        ));
    }

    header_spans.push(Span::styled(" ● ", Style::new().fg(gpu_color)));
    header_spans.push(Span::styled("GPU ", Style::new().fg(Theme::TEXT_SECONDARY)));
    header_spans.push(Span::styled(format!("{gpu_val} "), Style::new().fg(gpu_color).bold()));

    if let Some(mode) = gpu_mode {
        header_spans.push(Span::styled(
            format!("[{}]", mode.label()),
            Style::new().fg(fan_mode_color(mode)),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(header_spans)), header_area);

    // Prepare chart data
    let width = chart_area.width.saturating_sub(6) as usize; // Sub for y-axis labels
    let cpu_data = visible_history(cpu_history, width);
    let gpu_data = visible_history(gpu_history, width);

    let cpu_points: Vec<(f64, f64)> = cpu_data
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v as f64))
        .collect();
    let gpu_points: Vec<(f64, f64)> = gpu_data
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v as f64))
        .collect();

    let datasets = vec![
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(cpu_color))
            .data(&cpu_points),
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(gpu_color))
            .data(&gpu_points),
    ];

    let y_max = cpu_metric.max.max(gpu_metric.max);
    let chart = Chart::new(datasets)
        .block(Block::new().padding(Padding::new(1, 1, 0, 0)))
        .x_axis(
            Axis::default()
                .bounds([0.0, width as f64])
                .labels(vec![
                    Span::styled("Past", Style::new().fg(Theme::TEXT_TERTIARY)),
                    Span::styled("Now", Style::new().fg(Theme::TEXT_TERTIARY)),
                ]),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, y_max])
                .labels(vec![
                    Span::styled("0", Style::new().fg(Theme::TEXT_TERTIARY)),
                    Span::styled(format!("{:.0}", y_max / 2.0), Style::new().fg(Theme::TEXT_TERTIARY)),
                    Span::styled(format!("{:.0}", y_max), Style::new().fg(Theme::TEXT_TERTIARY)),
                ]),
        );

    frame.render_widget(chart, chart_area);
}

fn metric_value(metric: &AnimatedMetric, kind: MetricKind) -> String {
    if metric.target.is_none() {
        return "N/A".to_string();
    }

    match kind {
        MetricKind::Temp => format!("{:.0}°C", metric.value),
        MetricKind::Fan => format!("{:.0} RPM", metric.value),
    }
}

fn metric_sample_color(kind: MetricKind, value: f64, max: f64) -> Color {
    match kind {
        MetricKind::Temp => Theme::temp_color(value),
        MetricKind::Fan => Theme::fan_rpm_color(value, max),
    }
}

fn fan_mode_color(mode: FanMode) -> Color {
    match mode {
        FanMode::Auto => Theme::TEXT_SECONDARY,
        FanMode::Max => Theme::STATE_WARNING,
    }
}

fn visible_history(history: &VecDeque<u64>, width: usize) -> Vec<u64> {
    if width == 0 {
        return Vec::new();
    }

    let keep = width.min(history.len());
    history
        .iter()
        .skip(history.len().saturating_sub(keep))
        .copied()
        .collect()
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_set(symbols::border::DOUBLE)
        .border_style(Style::new().fg(Theme::BORDER_FRAME));

    frame.render_widget(block, area);

    let mut context_spans = vec![
        Span::styled(" [Tab] ", Style::new().bg(Theme::CHIP_BG).fg(Theme::CHIP_FG).bold()),
        Span::styled(" Switch Panel ", Style::new().fg(Theme::TEXT_SECONDARY)),
        Span::raw("   "),
        Span::styled(" [Q] ", Style::new().bg(Theme::CHIP_BG).fg(Theme::CHIP_FG).bold()),
        Span::styled(" Quit ", Style::new().fg(Theme::TEXT_SECONDARY)),
    ];

    let context_hints = app.context_hint();
    if !context_hints.is_empty() {
        context_spans.push(Span::styled("  |  ", Style::new().fg(Theme::BORDER_IDLE)));
    }

    for (key, action) in context_hints {
        context_spans.push(Span::styled(format!(" [{key}] "), Style::new().bg(Theme::CHIP_HIGHLIGHT_BG).fg(Theme::CHIP_HIGHLIGHT_FG).bold()));
        context_spans.push(Span::styled(format!(" {action} "), Style::new().fg(Theme::TEXT_PRIMARY)));
        context_spans.push(Span::raw("  "));
    }

    // Status message with simple colored indicator instead of a heavy chip
    context_spans.push(Span::styled("  |  ", Style::new().fg(Theme::BORDER_IDLE)));
    context_spans.push(Span::styled(" ● ", Style::new().fg(message_color(app.message.level))));
    context_spans.push(Span::styled(
        &app.message.text,
        Style::new().fg(Theme::TEXT_SECONDARY),
    ));

    if let Some(note) = &app.hardware_note {
        context_spans.push(Span::styled("  (", Style::new().fg(Theme::TEXT_DISABLED)));
        context_spans.push(Span::styled(
            note,
            Style::new().fg(Theme::TEXT_DISABLED),
        ));
        context_spans.push(Span::styled(")", Style::new().fg(Theme::TEXT_DISABLED)));
    }

    // Render content on row 2 (middle of the 5-row footer area)
    let content_area = Rect::new(area.x, area.y + 2, area.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(context_spans)).centered(),
        content_area,
    );
}

fn message_color(level: MessageLevel) -> Color {
    match level {
        MessageLevel::Info => Theme::STATE_INFO,
        MessageLevel::Success => Theme::STATE_SUCCESS,
        MessageLevel::Warning => Theme::STATE_WARNING,
        MessageLevel::Error => Theme::STATE_ERROR,
    }
}

fn control_state_color(applying: bool, pending: bool, error: bool) -> Color {
    if applying {
        Theme::STATE_INFO
    } else if error {
        Theme::STATE_ERROR
    } else if pending {
        Theme::STATE_WARNING
    } else {
        Theme::TEXT_DISABLED
    }
}

fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}
