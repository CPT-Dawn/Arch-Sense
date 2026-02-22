use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
};
use shared::{Command, FanMode, ProfessionalColor, Response, RgbMode};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/arch-sense.sock";

struct App {
    last_response: String,
    cpu_fan: u8,
    gpu_fan: u8,
    cpu_temp: u8,
    gpu_temp: u8,
    fan_mode: FanMode,
    rgb_mode: RgbMode,
    rgb_brightness: u8,
    fx_speed: u8,
    smart_battery_saver: bool,
    battery_limiter: bool,
}

fn get_temp_color(temp: u8) -> Color {
    match temp {
        0..=59 => Color::Green,
        60..=79 => Color::Yellow,
        _ => Color::Red,
    }
}

fn get_fan_color(speed: u8) -> Color {
    match speed {
        0..=34 => Color::Cyan,
        35..=69 => Color::Blue,
        _ => Color::Magenta,
    }
}

fn fan_mode_label(mode: &FanMode) -> String {
    match mode {
        FanMode::Auto => "Auto".to_string(),
        FanMode::Quiet => "Quiet".to_string(),
        FanMode::Balanced => "Balanced".to_string(),
        FanMode::Performance => "Performance".to_string(),
        FanMode::Turbo => "Turbo".to_string(),
        FanMode::Custom(cpu, gpu) => format!("Custom ({cpu}%/{gpu}%)"),
    }
}

fn rgb_mode_label(mode: &RgbMode) -> &'static str {
    match mode {
        RgbMode::Solid(ProfessionalColor::ArcticWhite) => "Solid · Arctic White",
        RgbMode::Solid(ProfessionalColor::ArchCyan) => "Solid · Arch Cyan",
        RgbMode::Solid(ProfessionalColor::NightShiftRed) => "Solid · Night-Shift Red",
        RgbMode::Solid(ProfessionalColor::EyeCareAmber) => "Solid · Eye-Care Amber",
        RgbMode::Wave => "Animation · Wave",
        RgbMode::Neon => "Animation · Neon",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        last_response: "Connecting to daemon...".to_string(),
        cpu_fan: 0,
        gpu_fan: 0,
        cpu_temp: 0,
        gpu_temp: 0,
        fan_mode: FanMode::Auto,
        rgb_mode: RgbMode::Solid(ProfessionalColor::ArchCyan),
        rgb_brightness: 70,
        fx_speed: 50,
        smart_battery_saver: false,
        battery_limiter: false,
    };

    let run_result = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = run_result {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| {
            let root = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(18),
                    Constraint::Length(3),
                ])
                .split(frame.size());

            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(root[1]);

            draw_header(frame, root[0]);
            draw_telemetry(frame, body[0], app);
            draw_right_panel(frame, body[1], app);
            draw_footer(frame, root[2], app);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('a') => {
                    app.last_response = send_command(Command::SetFanMode(FanMode::Auto)).await
                }
                KeyCode::Char('b') => {
                    app.last_response = send_command(Command::SetFanMode(FanMode::Balanced)).await
                }
                KeyCode::Char('t') => {
                    app.last_response = send_command(Command::SetFanMode(FanMode::Turbo)).await
                }
                KeyCode::Char('l') => {
                    app.last_response =
                        send_command(Command::SetBatteryLimiter(!app.battery_limiter)).await
                }
                KeyCode::Char('1') => {
                    app.last_response = send_command(Command::SetRgbMode(RgbMode::Solid(
                        ProfessionalColor::ArcticWhite,
                    )))
                    .await
                }
                KeyCode::Char('2') => {
                    app.last_response = send_command(Command::SetRgbMode(RgbMode::Solid(
                        ProfessionalColor::ArchCyan,
                    )))
                    .await
                }
                KeyCode::Char('3') => {
                    app.last_response = send_command(Command::SetRgbMode(RgbMode::Solid(
                        ProfessionalColor::NightShiftRed,
                    )))
                    .await
                }
                KeyCode::Char('4') => {
                    app.last_response = send_command(Command::SetRgbMode(RgbMode::Solid(
                        ProfessionalColor::EyeCareAmber,
                    )))
                    .await
                }
                KeyCode::Char('w') => {
                    app.last_response = send_command(Command::SetRgbMode(RgbMode::Wave)).await
                }
                KeyCode::Char('n') => {
                    app.last_response = send_command(Command::SetRgbMode(RgbMode::Neon)).await
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    app.last_response = send_command(Command::IncreaseRgbBrightness).await
                }
                KeyCode::Char('-') => {
                    app.last_response = send_command(Command::DecreaseRgbBrightness).await
                }
                KeyCode::Char('s') => {
                    app.last_response = send_command(Command::ToggleSmartBatterySaver).await
                }
                KeyCode::Char('o') => {
                    app.last_response = send_command(Command::SetLcdOverdrive(true)).await
                }
                KeyCode::Char('m') => {
                    app.last_response = send_command(Command::SetBootAnimation(false)).await
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            sync_status(app).await;
            last_tick = Instant::now();
        }
    }
}

fn draw_header(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " ARCH-SENSE ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled("Hardware Control Dashboard", Style::default().fg(Color::Gray)),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );

    frame.render_widget(header, area);
}

fn draw_telemetry(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    render_gauge(
        frame,
        top[0],
        " CPU Temperature ",
        app.cpu_temp,
        format!("{}°C", app.cpu_temp),
        get_temp_color(app.cpu_temp),
    );

    render_gauge(
        frame,
        top[1],
        " GPU Temperature ",
        app.gpu_temp,
        format!("{}°C", app.gpu_temp),
        get_temp_color(app.gpu_temp),
    );

    render_gauge(
        frame,
        bottom[0],
        " CPU Fan ",
        app.cpu_fan,
        format!("{}%", app.cpu_fan),
        get_fan_color(app.cpu_fan),
    );

    render_gauge(
        frame,
        bottom[1],
        " GPU Fan ",
        app.gpu_fan,
        format!("{}%", app.gpu_fan),
        get_fan_color(app.gpu_fan),
    );
}

fn draw_right_panel(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(12),
            Constraint::Min(10),
        ])
        .split(area);

    let status_lines = vec![
        Line::from(vec![
            Span::styled("Fan Mode: ", Style::default().fg(Color::Gray)),
            Span::styled(fan_mode_label(&app.fan_mode), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("RGB Mode: ", Style::default().fg(Color::Gray)),
            Span::styled(rgb_mode_label(&app.rgb_mode), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Brightness: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}%", app.rgb_brightness),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("FX Speed: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", app.fx_speed), Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::styled("Smart Saver: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if app.smart_battery_saver { "Enabled" } else { "Disabled" },
                Style::default().fg(if app.smart_battery_saver {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
    ];

    let status_block = Paragraph::new(status_lines)
        .block(
            Block::default()
                .title(" Live State ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .alignment(Alignment::Left);
    frame.render_widget(status_block, chunks[0]);

    let fan_power = Paragraph::new(vec![
        Line::from(Span::styled(
            "Fans/Power",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(" [a] Auto  [b] Balanced  [t] Turbo"),
        Line::from(" [l] Toggle Battery Limiter"),
    ])
    .block(
        Block::default()
            .title(" Hotkeys ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    frame.render_widget(fan_power, chunks[1]);

    let illumination_system = Paragraph::new(vec![
        Line::from(Span::styled(
            "Pro Illumination",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(" [1] Arctic White  [2] Arch Cyan"),
        Line::from(" [3] Night-Shift Red  [4] Eye-Care Amber"),
        Line::from(" [w] Wave  [n] Neon  [+/-] Brightness"),
        Line::from(""),
        Line::from(Span::styled(
            "System Settings",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(" [s] Toggle 30s Smart Saver"),
        Line::from(" [o] LCD Overdrive On   [m] Boot Sound Off"),
        Line::from(" [q] Quit"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );

    frame.render_widget(illumination_system, chunks[2]);
}

fn draw_footer(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let footer = Paragraph::new(app.last_response.clone())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" Daemon ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );

    frame.render_widget(footer, area);
}

fn render_gauge(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    title: &'static str,
    percent: u8,
    label: String,
    color: Color,
) {
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .gauge_style(Style::default().fg(color).bg(Color::Black))
        .percent(percent.min(100) as u16)
        .label(label);

    frame.render_widget(gauge, area);
}

async fn sync_status(app: &mut App) {
    let response = request(Command::GetHardwareStatus).await;
    if let Some(Response::HardwareStatus {
        cpu_temp,
        gpu_temp,
        cpu_fan_percent,
        gpu_fan_percent,
        fan_mode,
        active_rgb_mode,
        rgb_brightness,
        fx_speed,
        smart_battery_saver,
        battery_limiter,
    }) = response
    {
        app.cpu_temp = cpu_temp;
        app.gpu_temp = gpu_temp;
        app.cpu_fan = cpu_fan_percent;
        app.gpu_fan = gpu_fan_percent;
        app.fan_mode = fan_mode;
        app.rgb_mode = active_rgb_mode;
        app.rgb_brightness = rgb_brightness;
        app.fx_speed = fx_speed;
        app.smart_battery_saver = smart_battery_saver;
        app.battery_limiter = battery_limiter;
    }
}

async fn send_command(command: Command) -> String {
    match request(command).await {
        Some(Response::Ack(message)) => format!("✓ {message}"),
        Some(Response::Error(err)) => format!("✗ {err}"),
        Some(Response::HardwareStatus { .. }) => "Status synced".to_string(),
        None => "Unable to communicate with daemon".to_string(),
    }
}

async fn request(command: Command) -> Option<Response> {
    let mut stream = UnixStream::connect(SOCKET_PATH).await.ok()?;
    let message = serde_json::to_vec(&command).ok()?;

    if stream.write_all(&message).await.is_err() {
        return None;
    }

    let mut buffer = vec![0; 4096];
    let read_bytes = stream.read(&mut buffer).await.ok()?;
    if read_bytes == 0 {
        return None;
    }

    serde_json::from_slice(&buffer[..read_bytes]).ok()
}
