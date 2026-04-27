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
    // This enables transparency/blur support and respects the user's terminal theme
    let base_style = match Theme::BG {
        Some(bg) => Style::new().bg(bg),
        None => Style::new(),
    };
    frame.render_widget(Block::new().style(base_style), area);

    let [header, body, footer] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(18),
        Constraint::Length(4),
    ])
    .margin(SPACING)
    .areas(area);

    draw_header(frame, header);
    draw_body(frame, body, app);
    draw_footer(frame, footer, app);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App) {
    let [left, _, right] = Layout::horizontal([
        Constraint::Percentage(42),
        Constraint::Length(SPACING),
        Constraint::Percentage(58),
    ])
    .areas(area);

    let [controls, _, rgb] = Layout::vertical([
        Constraint::Percentage(58),
        Constraint::Length(SPACING),
        Constraint::Percentage(42),
    ])
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

    let mut title_spans = vec![Span::styled(format!(" {title} "), title_style)];

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
    let block = Block::bordered()
        .border_set(DOUBLE_SQUIRCLE_BORDER)
        .border_style(Style::new().fg(Theme::BORDER_FRAME));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let [title_area] = Layout::vertical([Constraint::Length(1)])
        .flex(ratatui::layout::Flex::Center)
        .areas(inner);

    let title = Line::from(vec![
        Span::styled("  ◆ ", Style::new().fg(Theme::BRAND_PRIMARY).bold()),
        Span::styled(
            "A R C H - S E N S E",
            Style::new().fg(Theme::BRAND_PRIMARY).bold(),
        ),
        Span::styled("  ◆  ", Style::new().fg(Theme::BRAND_SECONDARY)),
        Span::styled(
            "Acer Predator Control Center",
            Style::new().fg(Theme::TEXT_SECONDARY),
        ),
    ])
    .centered();

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
            let base_style = if selected {
                style_with_bg(Style::new().fg(Theme::TEXT_PRIMARY).bold(), Theme::ELEVATED)
            } else {
                Style::new().fg(Theme::TEXT_PRIMARY)
            };
            let value_style = if error {
                let base = Style::new().fg(Theme::STATE_ERROR);
                let bg = if selected {
                    Theme::ELEVATED
                } else {
                    Theme::SURFACE
                };
                style_with_bg(base, bg)
            } else if pending {
                let base = Style::new().fg(Theme::STATE_WARNING).bold();
                let bg = if selected {
                    Theme::ELEVATED
                } else {
                    Theme::SURFACE
                };
                style_with_bg(base, bg)
            } else {
                Style::new().fg(if selected {
                    Theme::VALUE_SELECTED
                } else {
                    Theme::VALUE_PRIMARY
                })
            };
            let marker = if selected { "▸" } else { " " };
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
            ])
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

    let [rows_area, palette_area, preview_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .spacing(SPACING)
    .areas(content_area);

    draw_rgb_rows(frame, rows_area, app);
    draw_palette(frame, palette_area, app);
    draw_rgb_preview(frame, preview_area, app);
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

    let lines = fields
        .into_iter()
        .enumerate()
        .map(|(index, (field, value))| {
            let selected = app.focus == FocusPanel::Rgb && index == app.selected_rgb_field;
            let style = if selected {
                style_with_bg(Style::new().fg(Theme::TEXT_PRIMARY).bold(), Theme::ELEVATED)
            } else {
                Style::new().fg(Theme::TEXT_PRIMARY)
            };
            let value_style = if selected {
                style_with_bg(
                    Style::new().fg(Theme::VALUE_SELECTED).bold(),
                    Theme::ELEVATED,
                )
            } else {
                Style::new().fg(Theme::VALUE_PRIMARY)
            };

            Line::from(vec![
                Span::styled(if selected { "▸ " } else { "  " }, style),
                Span::styled(format!("{:<11}", field.label()), style),
                Span::styled(value, value_style),
            ])
        })
        .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(lines), area);
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
        " Palette ",
        Style::new().fg(Theme::TEXT_SECONDARY),
    )];
    for (index, color) in COLOR_PALETTE.iter().enumerate() {
        let selected = index == app.rgb.color_idx;
        let bg = if selected {
            Theme::ELEVATED
        } else {
            Theme::SURFACE
        };
        let style = if index == RANDOM_COLOR_INDEX {
            style_with_bg(Style::new().fg(Theme::BRAND_TERTIARY).bold(), bg)
        } else {
            style_with_bg(Style::new().fg(to_color(color.rgb)).bold(), bg)
        };
        swatches.push(Span::styled(if selected { "▣" } else { "■" }, style));
        swatches.push(Span::raw(" "));
    }

    frame.render_widget(Paragraph::new(Line::from(swatches)), area);
}

fn draw_rgb_preview(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();
    let status = match &app.keyboard {
        UsbAccess::Accessible => {
            if app.rgb_pending {
                Span::styled("Applying", Style::new().fg(Theme::STATE_WARNING).bold())
            } else if app.rgb_dirty {
                Span::styled("Preview", Style::new().fg(Theme::STATE_INFO).bold())
            } else {
                Span::styled("Ready", Style::new().fg(Theme::STATE_SUCCESS).bold())
            }
        }
        UsbAccess::PermissionDenied => {
            Span::styled("USB locked", Style::new().fg(Theme::STATE_WARNING).bold())
        }
        UsbAccess::NotFound => Span::styled(
            "Keyboard missing",
            Style::new().fg(Theme::STATE_WARNING).bold(),
        ),
        UsbAccess::Error(_) => {
            Span::styled("USB error", Style::new().fg(Theme::STATE_ERROR).bold())
        }
    };

    lines.push(Line::from(vec![
        Span::styled(" State ", Style::new().fg(Theme::TEXT_SECONDARY)),
        status,
    ]));
    lines.push(Line::from(rgb_preview_spans(
        app,
        area.width.saturating_sub(2) as usize,
    )));

    frame.render_widget(Paragraph::new(lines), area);
}

fn rgb_preview_spans(app: &App, width: usize) -> Vec<Span<'static>> {
    let effect = app.rgb.effect();
    let width = width.clamp(12, 48);
    let mut spans = Vec::with_capacity(width);

    for i in 0..width {
        let color = if app.rgb.effect_idx == 0 {
            Theme::TEXT_DISABLED
        } else if effect.has_color && app.rgb.color_idx != RANDOM_COLOR_INDEX {
            to_color(app.rgb.color().rgb)
        } else {
            let offset = ((app.rgb_phase as usize / 2) + i) % (COLOR_PALETTE.len() - 1);
            to_color(COLOR_PALETTE[offset].rgb)
        };
        spans.push(Span::styled("━", Style::new().fg(color).bold()));
    }

    spans
}

fn draw_sensors(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Sensors", FocusPanel::Sensors, app);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Apply inner padding/margin to the content area
    let content_area = Layout::vertical([Constraint::Min(0)])
        .margin(SPACING)
        .split(inner)[0];

    let [cpu_t, gpu_t, cpu_f, gpu_f] = Layout::vertical([
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .spacing(SPACING)
    .areas(content_area);

    draw_metric(
        frame,
        cpu_t,
        "CPU Temperature",
        &app.sensors.cpu_temp,
        &app.sensors.cpu_temp_history,
        MetricKind::Temp,
        None,
    );
    draw_metric(
        frame,
        gpu_t,
        "GPU Temperature",
        &app.sensors.gpu_temp,
        &app.sensors.gpu_temp_history,
        MetricKind::Temp,
        None,
    );
    draw_metric(
        frame,
        cpu_f,
        "CPU Fan",
        &app.sensors.cpu_fan,
        &app.sensors.cpu_fan_history,
        MetricKind::Fan,
        Some(app.sensors.cpu_fan_mode),
    );
    draw_metric(
        frame,
        gpu_f,
        "GPU Fan",
        &app.sensors.gpu_fan,
        &app.sensors.gpu_fan_history,
        MetricKind::Fan,
        Some(app.sensors.gpu_fan_mode),
    );
}

#[derive(Clone, Copy)]
enum MetricKind {
    Temp,
    Fan,
}

fn draw_metric(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    metric: &AnimatedMetric,
    history: &VecDeque<u64>,
    kind: MetricKind,
    mode: Option<FanMode>,
) {
    if area.height < 2 {
        return;
    }

    let [top, spark_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);

    let value = metric_value(metric, kind);
    let color = metric_sample_color(kind, metric.value, metric.max);

    let mut top_spans = vec![Span::styled(
        format!(" {label:<17}"),
        Style::new().fg(Theme::TEXT_PRIMARY).bold(),
    )];

    if let Some(mode) = mode {
        top_spans.push(Span::styled(
            format!("{} ", mode.label()),
            Style::new().fg(fan_mode_color(mode)).bold(),
        ));
    }

    top_spans.push(Span::styled(value, Style::new().fg(color).bold()));

    if metric.error.is_some() {
        top_spans.push(Span::styled(
            " N/A",
            Style::new().fg(Theme::STATE_ERROR).bold(),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(top_spans)), top);

    let width = spark_area.width.max(1) as usize;
    let mut data = visible_history(history, width);
    if data.is_empty() {
        data = vec![0; width];
    }

    let bar_color = if metric.error.is_some() {
        Theme::TEXT_DISABLED
    } else {
        color
    };

    let bars = data
        .into_iter()
        .map(|value| {
            let sample_color = if metric.error.is_some() {
                bar_color
            } else {
                metric_sample_color(kind, value as f64, metric.max)
            };
            SparklineBar::from(value).style(Some(Style::new().fg(sample_color)))
        })
        .collect::<Vec<_>>();

    let sparkline = Sparkline::default()
        .data(bars)
        .max(metric.max.round() as u64)
        .bar_set(symbols::bar::NINE_LEVELS)
        .style(style_with_bg(Style::new(), Theme::SURFACE));
    frame.render_widget(sparkline, spark_area);
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
    let mut block = Block::bordered()
        .border_set(DOUBLE_SQUIRCLE_BORDER)
        .border_style(Style::new().fg(Theme::BORDER_FRAME));

    // Apply optional background
    block = match Theme::SURFACE {
        Some(bg) => block.style(Style::new().bg(bg)),
        None => block,
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [context, message] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)])
        .spacing(SPACING)
        .areas(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Tab switch panels", Style::new().fg(Theme::TEXT_SECONDARY)),
            Span::styled(
                "  |  r refresh  |  ",
                Style::new().fg(Theme::TEXT_SECONDARY),
            ),
            Span::styled("q quit", Style::new().fg(Theme::TEXT_SECONDARY)),
            Span::styled("  |  ", Style::new().fg(Theme::TEXT_DISABLED)),
            Span::styled(app.context_hint(), Style::new().fg(Theme::TEXT_PRIMARY)),
        ])),
        context,
    );

    let mut message_spans = vec![
        {
            let level_style = style_with_bg(
                Style::new()
                    .fg(Theme::TEXT_ON_STATUS)
                    .bg(message_color(app.message.level))
                    .bold(),
                None,
            );
            Span::styled(
                format!(" {} ", message_level(app.message.level)),
                level_style,
            )
        },
        Span::raw(" "),
        Span::styled(
            &app.message.text,
            Style::new().fg(message_color(app.message.level)),
        ),
    ];

    if let Some(note) = &app.hardware_note {
        message_spans.push(Span::styled("  |  ", Style::new().fg(Theme::TEXT_DISABLED)));
        message_spans.push(Span::styled(
            note,
            Style::new().fg(Theme::TEXT_SECONDARY),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(message_spans)), message);
}

fn message_level(level: MessageLevel) -> &'static str {
    match level {
        MessageLevel::Info => "INFO",
        MessageLevel::Success => "OK",
        MessageLevel::Warning => "WARN",
        MessageLevel::Error => "ERR",
    }
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
