//! The four gauge cards: CPU, GPU, Fans and Battery.

use std::collections::VecDeque;

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, LineGauge, Paragraph, Sparkline};

use crate::app::App;
use crate::capabilities::Capability;
use crate::settings::SettingId;
use crate::theme::Theme;

/// Fan speed that fills the Fans gauge.
const FAN_FULL_RPM: f64 = 6000.0;
/// Sparklines plot temperatures between these.
const SPARK_FLOOR: u64 = 30;
const SPARK_CEILING: u64 = 100;

/// What fills a card below its headline value.
enum Body<'a> {
    Sparkline(&'a VecDeque<u64>),
    Gauge(f64),
    Text(Text<'static>),
}

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let [cpu, gpu, fans, battery] = Layout::horizontal([Constraint::Fill(1); 4]).areas(area);
    let sensors = &app.sensors;

    let read = sensors.read;
    temperature_card(
        frame,
        cpu,
        "CPU",
        sensors.cpu_temp,
        &sensors.cpu_history,
        read,
    );
    temperature_card(
        frame,
        gpu,
        "GPU",
        sensors.gpu_temp,
        &sensors.gpu_history,
        read,
    );

    let mode = app
        .settings
        .control(SettingId::Hardware(Capability::Fans))
        .map(|control| control.value_text())
        .or_else(|| sensors.fan_mode.map(|mode| mode.label().to_string()));
    let title = match mode {
        Some(mode) => format!("Fans · {mode}"),
        None => "Fans".to_string(),
    };
    let rpm = |rpm: Option<f64>| rpm.map_or("—".to_string(), |rpm| format!("{rpm:.0}"));
    let value = match (sensors.cpu_fan, sensors.gpu_fan) {
        (None, None) => None,
        (cpu, gpu) => Some(format!("{} / {} rpm", rpm(cpu), rpm(gpu))),
    };
    let fastest = sensors
        .cpu_fan
        .unwrap_or(0.0)
        .max(sensors.gpu_fan.unwrap_or(0.0));
    card(
        frame,
        fans,
        &title,
        value,
        Theme::ACCENT_ALT,
        Body::Gauge(fastest / FAN_FULL_RPM),
    );

    let limit = app
        .settings
        .control(SettingId::Hardware(Capability::ChargeLimit))
        .map(|control| match control.value_text().as_str() {
            "On" => "limit 80%",
            _ => "no limit",
        });
    battery_card(frame, battery, app, limit);
}

/// Charge, state, watts and the charge limit; "No battery" when there's none.
fn battery_card(frame: &mut Frame, area: Rect, app: &App, limit: Option<&str>) {
    let dim = Style::new().fg(Theme::DIM);
    let Some(battery) = app.sensors.battery else {
        let body = if app.sensors.read { "No battery" } else { "" };
        card(
            frame,
            area,
            "Battery",
            None,
            Theme::OK,
            Body::Text(Line::styled(body, dim).into()),
        );
        return;
    };

    let state = match battery.watts {
        Some(watts) if watts >= 0.05 => format!("{} {watts:.1} W", battery.state.label()),
        _ => battery.state.label().to_string(),
    };
    let lines = vec![
        Line::styled(state, Style::new().fg(Theme::TEXT)),
        Line::styled(limit.unwrap_or_default().to_string(), dim),
    ];
    let colour = match battery.percent {
        0..=15 => Theme::ERROR,
        16..=30 => Theme::WARN,
        _ => Theme::OK,
    };
    card(
        frame,
        area,
        "Battery",
        Some(format!("{}%", battery.percent)),
        colour,
        Body::Text(Text::from(lines)),
    );
}

fn temperature_card(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    celsius: Option<f64>,
    history: &VecDeque<u64>,
    read: bool,
) {
    let colour = celsius.map_or(Theme::DIM, Theme::temperature);
    let value = celsius.map(|celsius| format!("{celsius:.0}°C"));
    // Before the first scan the card is just empty; after it, say why.
    let body = if celsius.is_none() && read {
        Body::Text(Line::styled("unavailable", Style::new().fg(Theme::DIM)).into())
    } else {
        Body::Sparkline(history)
    };
    card(frame, area, title, value, colour, body);
}

fn card(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    value: Option<String>,
    colour: Color,
    body: Body,
) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::LINE))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(Theme::DIM),
        ));
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);

    let [headline, rest] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    let headline_text = match value {
        Some(value) => Span::styled(value, Style::new().fg(colour).bold()),
        None => Span::styled("—", Style::new().fg(Theme::DIM)),
    };
    frame.render_widget(Paragraph::new(headline_text), headline);

    match body {
        Body::Sparkline(history) => {
            let data: Vec<u64> = history
                .iter()
                .rev()
                .take(rest.width as usize)
                .rev()
                .map(|value| value.saturating_sub(SPARK_FLOOR))
                .collect();
            frame.render_widget(
                Sparkline::default()
                    .data(&data)
                    .max(SPARK_CEILING - SPARK_FLOOR)
                    .style(Style::new().fg(colour)),
                rest,
            );
        }
        Body::Gauge(ratio) => {
            let bottom = Rect {
                y: rest.bottom().saturating_sub(1),
                height: rest.height.min(1),
                ..rest
            };
            frame.render_widget(
                LineGauge::default()
                    .ratio(ratio.clamp(0.0, 1.0))
                    .filled_style(Style::new().fg(colour))
                    .unfilled_style(Style::new().fg(Theme::LINE))
                    .label(""),
                bottom,
            );
        }
        Body::Text(text) => frame.render_widget(Paragraph::new(text), rest),
    }
}
