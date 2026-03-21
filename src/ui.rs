use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::app::{App, Tab};
use crate::rgb_settings::{COLOR_PALETTE, EFFECTS, RANDOM_COLOR_IDX, SettingKind};
use crate::theme::Theme;

pub(crate) fn draw(f: &mut Frame, app: &App) {
    let [header, tab_bar, body, detail, status] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(12),
        Constraint::Length(6),
        Constraint::Length(3),
    ])
    .areas(f.area());

    draw_header(f, header);
    draw_tab_bar(f, tab_bar, app);

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(body);

    draw_sensors(f, left, app);

    match app.tab {
        Tab::System => {
            draw_controls(f, right, app);
            draw_detail(f, detail, app);
        }
        Tab::Rgb => {
            draw_rgb_panel(f, right, app);
            draw_rgb_detail(f, detail, app);
        }
    }

    draw_status(f, status, app);
}

// ─── Header ─────────────────────────────────────────────────────────────────

fn draw_header(f: &mut Frame, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(Theme::ACCENT))
        .style(Style::new().bg(Theme::BG_HEADER));

    let text = Line::from(vec![
        Span::styled("  ◆ ", Style::new().fg(Theme::ACCENT).bold()),
        Span::styled("A R C H - S E N S E", Style::new().fg(Theme::ACCENT).bold()),
        Span::styled("  ◆  ", Style::new().fg(Theme::ACCENT)),
        Span::styled(
            "Acer Predator Control Center",
            Style::new().fg(Theme::FG_DIM),
        ),
    ])
    .centered();

    f.render_widget(Paragraph::new(text).block(block), area);
}

// ─── Tab Bar ────────────────────────────────────────────────────────────────

fn draw_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    let sys = if app.tab == Tab::System {
        Style::new().fg(Color::Black).bg(Theme::ACCENT).bold()
    } else {
        Style::new().fg(Theme::FG_DIM)
    };
    let rgb = if app.tab == Tab::Rgb {
        Style::new().fg(Color::Black).bg(Theme::ACCENT).bold()
    } else {
        Style::new().fg(Theme::FG_DIM)
    };

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(" F1 System ", sys),
        Span::raw("  "),
        Span::styled(" F2 Keyboard RGB ", rgb),
        Span::styled(
            "                              Tab to switch",
            Style::new().fg(Theme::DARK),
        ),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

// ─── Sensor Bars ────────────────────────────────────────────────────────────

fn make_bar(val: f64, max: f64, w: u16) -> Line<'static> {
    let ratio = (val / max).clamp(0.0, 1.0);
    let fill = (ratio * w as f64) as usize;
    let empty = (w as usize).saturating_sub(fill);
    let color = if ratio < 0.55 {
        Theme::COOL
    } else if ratio < 0.78 {
        Theme::WARM
    } else {
        Theme::HOT
    };
    Line::from(vec![
        Span::raw("  "),
        Span::styled("━".repeat(fill), Style::new().fg(color)),
        Span::styled("─".repeat(empty), Style::new().fg(Theme::DARK)),
    ])
}

// ─── Sensors Panel ──────────────────────────────────────────────────────────

fn draw_sensors(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " Sensors ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let bar_w = inner.width.saturating_sub(4);

    let sl = |label: &str, val: String, color: Color| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {:<18}", label), Style::new().fg(Theme::FG)),
            Span::styled(val, Style::new().fg(color).bold()),
        ])
    };

    let cpu_t = app.sensors.cpu_t.unwrap_or(0.0);
    let cpu_s = app
        .sensors
        .cpu_t
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or("N/A".into());
    let cpu_c = app
        .sensors
        .cpu_t
        .map(Theme::temp_color)
        .unwrap_or(Theme::FG_DIM);

    let gpu_t = app.sensors.gpu_t.unwrap_or(0.0);
    let gpu_s = app
        .sensors
        .gpu_t
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or("N/A".into());
    let gpu_c = app
        .sensors
        .gpu_t
        .map(Theme::temp_color)
        .unwrap_or(Theme::FG_DIM);

    let cf = app.sensors.cpu_f.unwrap_or(0);
    let cf_s = app
        .sensors
        .cpu_f
        .map(|p| {
            if p == 0 {
                "Auto".into()
            } else {
                format!("{p}%")
            }
        })
        .unwrap_or("N/A".into());
    let cf_c = app
        .sensors
        .cpu_f
        .map(Theme::fan_color)
        .unwrap_or(Theme::FG_DIM);

    let gf = app.sensors.gpu_f.unwrap_or(0);
    let gf_s = app
        .sensors
        .gpu_f
        .map(|p| {
            if p == 0 {
                "Auto".into()
            } else {
                format!("{p}%")
            }
        })
        .unwrap_or("N/A".into());
    let gf_c = app
        .sensors
        .gpu_f
        .map(Theme::fan_color)
        .unwrap_or(Theme::FG_DIM);

    let lines = vec![
        sl("CPU Temperature", cpu_s, cpu_c),
        make_bar(cpu_t, 105.0, bar_w),
        Line::default(),
        sl("GPU Temperature", gpu_s, gpu_c),
        make_bar(gpu_t, 105.0, bar_w),
        Line::default(),
        sl("CPU Fan", cf_s, cf_c),
        make_bar(cf as f64, 100.0, bar_w),
        Line::default(),
        sl("GPU Fan", gf_s, gf_c),
        make_bar(gf as f64, 100.0, bar_w),
    ];

    f.render_widget(Paragraph::new(lines), inner);
}

// ─── Controls Panel ─────────────────────────────────────────────────────────

fn draw_controls(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " Controls ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.settings.is_empty() {
        f.render_widget(
            Paragraph::new("No settings available")
                .style(Style::new().fg(Theme::FG_DIM))
                .centered(),
            inner,
        );
        return;
    }

    let rows: Vec<Row> = app
        .settings
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let sel = i == app.ctrl_sel;
            let arrow = if sel { " ▸ " } else { "   " };
            let style = if sel {
                Style::new().fg(Theme::ACCENT).bg(Theme::BG_HL).bold()
            } else {
                Style::new().fg(Theme::FG)
            };

            // Show pending preview if cycling, else show current
            let disp = if let Some(pidx) = s.pending {
                if let SettingKind::Cycle(ref opts) = s.kind {
                    opts.get(pidx)
                        .map(|o| format!("◀ {} ▶", o.label))
                        .unwrap_or(s.display.clone())
                } else {
                    s.display.clone()
                }
            } else {
                s.display.clone()
            };

            let val_style = if sel && s.pending.is_some() {
                Style::new().fg(Theme::WARM).bg(Theme::BG_HL).bold()
            } else if sel {
                Style::new().fg(Theme::ACCENT2).bg(Theme::BG_HL).bold()
            } else {
                Style::new().fg(Theme::DIM)
            };

            let hint = match (&s.kind, sel) {
                (SettingKind::Toggle, true) => " [Enter]",
                (SettingKind::Cycle(_), true) if s.pending.is_some() => " [Enter]",
                (SettingKind::Cycle(_), true) => " [←→]",
                _ => "",
            };

            Row::new(vec![
                Cell::new(arrow).style(style),
                Cell::new(format!("{:<20}", s.label)).style(style),
                Cell::new(disp).style(val_style),
                Cell::new(hint).style(Style::new().fg(Theme::FG_DIM)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(21),
        Constraint::Min(14),
        Constraint::Length(9),
    ];

    f.render_widget(Table::new(rows, widths).column_spacing(0), inner);
}

// ─── RGB Panel ──────────────────────────────────────────────────────────────

fn draw_rgb_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " Keyboard RGB ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if !app.rgb.kb_found {
        let msg = vec![
            Line::default(),
            Line::from(Span::styled(
                "  ⚠ No compatible keyboard detected",
                Style::new().fg(Theme::WARM),
            )),
            Line::from(Span::styled(
                "    Expected: Acer Predator PH16-71 (04F2:0117)",
                Style::new().fg(Theme::FG_DIM),
            )),
            Line::default(),
            Line::from(Span::styled(
                "    Config can still be edited & saved.",
                Style::new().fg(Theme::DIM),
            )),
            Line::from(Span::styled(
                "    Keyboard will be detected when plugged in.",
                Style::new().fg(Theme::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(msg), inner);
        return;
    }

    let eff = app.rgb.eff();
    let bar_w: usize = 20;

    let mk_row = |idx: usize, label: &str, spans: Vec<Span<'static>>| -> Vec<Line<'static>> {
        let sel = idx == app.rgb.sel;
        let arr = if sel { " ▸ " } else { "   " };
        let ls = if sel {
            Style::new().fg(Theme::ACCENT).bold()
        } else {
            Style::new().fg(Theme::FG)
        };
        let mut all = vec![
            Span::styled(String::from(arr), ls),
            Span::styled(format!("{:<14}", label), ls),
        ];
        all.extend(spans);
        vec![Line::from(all)]
    };

    // Effect
    let effect_spans = vec![
        Span::styled("◀ ", Style::new().fg(Theme::DIM)),
        Span::styled(
            String::from(eff.name),
            Style::new().fg(Theme::ACCENT2).bold(),
        ),
        Span::styled(" ▶", Style::new().fg(Theme::DIM)),
    ];

    // Color
    let c = app.rgb.color_rgb();
    let cn = app.rgb.color_name();
    let color_spans = if eff.has_color {
        let swatch = if app.rgb.color_idx == RANDOM_COLOR_IDX {
            Span::styled(" ◆◆◆ ", Style::new().fg(Theme::ACCENT))
        } else {
            Span::styled(" ███ ", Style::new().fg(Color::Rgb(c.r, c.g, c.b)))
        };
        vec![
            Span::styled("◀ ", Style::new().fg(Theme::DIM)),
            Span::styled(String::from(cn), Style::new().fg(Theme::ACCENT2).bold()),
            Span::styled(" ▶ ", Style::new().fg(Theme::DIM)),
            swatch,
        ]
    } else {
        vec![Span::styled(
            "  N/A (effect has no color)",
            Style::new().fg(Theme::DARK),
        )]
    };

    // Brightness bar
    let bf = (app.rgb.brightness as usize * bar_w / 100).min(bar_w);
    let be = bar_w.saturating_sub(bf);
    let bright_spans = vec![
        Span::styled("━".repeat(bf), Style::new().fg(Theme::ACCENT)),
        Span::styled("─".repeat(be), Style::new().fg(Theme::DARK)),
        Span::styled(
            format!(" {}%", app.rgb.brightness),
            Style::new().fg(Theme::FG).bold(),
        ),
    ];

    // Speed bar
    let sf = (app.rgb.speed as usize * bar_w / 100).min(bar_w);
    let se = bar_w.saturating_sub(sf);
    let speed_spans = vec![
        Span::styled("━".repeat(sf), Style::new().fg(Theme::ACCENT)),
        Span::styled("─".repeat(se), Style::new().fg(Theme::DARK)),
        Span::styled(
            format!(" {}%", app.rgb.speed),
            Style::new().fg(Theme::FG).bold(),
        ),
    ];

    // Direction
    let dir_spans = if eff.has_dir {
        vec![
            Span::styled("◀ ", Style::new().fg(Theme::DIM)),
            Span::styled(
                String::from(app.rgb.dir_name()),
                Style::new().fg(Theme::ACCENT2).bold(),
            ),
            Span::styled(" ▶", Style::new().fg(Theme::DIM)),
        ]
    } else {
        vec![Span::styled(
            "  N/A (Wave only)",
            Style::new().fg(Theme::DARK),
        )]
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.extend(mk_row(0, "Effect", effect_spans));
    lines.push(Line::default());
    lines.extend(mk_row(1, "Color", color_spans));
    lines.push(Line::default());
    lines.extend(mk_row(2, "Brightness", bright_spans));
    lines.push(Line::default());
    lines.extend(mk_row(3, "Speed", speed_spans));
    lines.push(Line::default());
    lines.extend(mk_row(4, "Direction", dir_spans));

    f.render_widget(Paragraph::new(lines), inner);
}

// ─── Detail Panel (System Tab) ──────────────────────────────────────────────

fn draw_detail(f: &mut Frame, area: Rect, app: &App) {
    if app.settings.is_empty() {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(Theme::DARK))
            .title(Span::styled(
                " Details ",
                Style::new().fg(Theme::ACCENT).bold(),
            ));
        f.render_widget(Paragraph::new("  No settings loaded").block(block), area);
        return;
    }

    let s = &app.settings[app.ctrl_sel];
    let border = if s.pending.is_some() {
        Theme::WARM
    } else {
        Theme::DIM
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(Span::styled(
            format!(" {} ", s.label),
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let mut lines = vec![
        Line::from(vec![
            Span::styled("  Current: ", Style::new().fg(Theme::FG_DIM)),
            Span::styled(s.display.clone(), Style::new().fg(Theme::ACCENT).bold()),
            Span::styled("  │  Raw: ", Style::new().fg(Theme::FG_DIM)),
            Span::styled(s.raw.clone(), Style::new().fg(Theme::FG)),
        ]),
        Line::from(Span::styled(
            format!("  {}", s.desc),
            Style::new().fg(Theme::FG).italic(),
        )),
    ];

    if let Some(pidx) = s.pending
        && let SettingKind::Cycle(ref opts) = s.kind
        && let Some(opt) = opts.get(pidx)
    {
        lines.push(Line::from(vec![
            Span::styled("  Preview: ", Style::new().fg(Theme::WARM)),
            Span::styled(opt.label.clone(), Style::new().fg(Theme::WARM).bold()),
            Span::styled("  → Enter to apply", Style::new().fg(Theme::FG_DIM)),
        ]));
    }

    let hint = match &s.kind {
        SettingKind::Toggle => "  Enter: Toggle  │  ↑↓: Navigate".into(),
        SettingKind::Cycle(opts) => {
            let names: Vec<&str> = opts.iter().map(|o| o.label.as_str()).collect();
            format!("  ←→: [{}]  │  Enter: Confirm", names.join(" │ "))
        }
    };
    lines.push(Line::from(Span::styled(hint, Style::new().fg(Theme::DIM))));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

// ─── Detail Panel (RGB Tab) ─────────────────────────────────────────────────

fn draw_rgb_detail(f: &mut Frame, area: Rect, app: &App) {
    let eff = app.rgb.eff();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DIM))
        .title(Span::styled(
            " RGB Details ",
            Style::new().fg(Theme::ACCENT).bold(),
        ));

    let desc = match app.rgb.sel {
        0 => format!(
            "  {} — {}/{} effects. ←→ to browse.",
            eff.name,
            app.rgb.effect_idx + 1,
            EFFECTS.len()
        ),
        1 => format!(
            "  {} — {}/{} colors. ←→ to cycle.",
            app.rgb.color_name(),
            app.rgb.color_idx + 1,
            COLOR_PALETTE.len()
        ),
        2 => format!(
            "  Brightness {}% — LED intensity. ←→ adjusts ±10%.",
            app.rgb.brightness
        ),
        3 => format!(
            "  Speed {}% — Animation speed (100 = fastest). ←→ adjusts ±10%.",
            app.rgb.speed
        ),
        4 => format!("  {} — Wave direction. ←→ to cycle.", app.rgb.dir_name()),
        _ => String::new(),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("  Preview: ", Style::new().fg(Theme::FG_DIM)),
            Span::styled(
                String::from(eff.name),
                Style::new().fg(Theme::ACCENT2).bold(),
            ),
            if eff.has_color {
                Span::styled(
                    format!(" │ {} ", app.rgb.color_name()),
                    Style::new().fg(Theme::FG),
                )
            } else {
                Span::raw("")
            },
            Span::styled(
                format!("│ B:{}% S:{}%", app.rgb.brightness, app.rgb.speed),
                Style::new().fg(Theme::FG),
            ),
            if eff.has_dir {
                Span::styled(
                    format!(" │ Dir:{}", app.rgb.dir_name()),
                    Style::new().fg(Theme::FG),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(Span::styled(desc, Style::new().fg(Theme::FG_DIM))),
        Line::default(),
        Line::from(Span::styled(
            "  Enter: Apply (auto-saves)  │  ←→: Adjust  │  ↑↓: Param",
            Style::new().fg(Theme::DIM),
        )),
    ];

    f.render_widget(Paragraph::new(lines).block(block), area);
}

// ─── Status Bar ─────────────────────────────────────────────────────────────

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let tab_span = match app.tab {
        Tab::System => Span::styled(
            " SYSTEM ",
            Style::new().fg(Color::Black).bg(Theme::ACCENT).bold(),
        ),
        Tab::Rgb => Span::styled(
            " RGB ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Rgb(128, 0, 255))
                .bold(),
        ),
    };

    let module_span = if app.module_ok {
        Span::styled(" MODULE ✓ ", Style::new().fg(Theme::COOL).bold())
    } else {
        Span::styled(" NO MODULE ", Style::new().fg(Theme::ERR).bold())
    };

    let kb_span = if app.rgb.kb_found {
        Span::styled(" KB ✓ ", Style::new().fg(Theme::COOL).bold())
    } else {
        Span::styled(" NO KB ", Style::new().fg(Theme::WARM).bold())
    };

    let sc = if app.err { Theme::ERR } else { Theme::FG_DIM };

    let help = match app.tab {
        Tab::System => " F1/F2 Tab │ ↑↓ Navigate │ ←→ Cycle │ Enter Confirm/Toggle │ q Quit ",
        Tab::Rgb => " F1/F2 Tab │ ↑↓ Param │ ←→ Adjust │ Enter Apply (auto-save) │ q Quit ",
    };

    let lines = vec![
        Line::from(vec![
            tab_span,
            Span::raw(" "),
            module_span,
            kb_span,
            Span::raw(" "),
            Span::styled(app.status.clone(), Style::new().fg(sc)),
        ]),
        Line::from(Span::styled(help, Style::new().fg(Theme::FG_DIM))),
    ];

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Theme::DARK));

    f.render_widget(Paragraph::new(lines).block(block), area);
}
