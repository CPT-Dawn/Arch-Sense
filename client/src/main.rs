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
    thermal_profile: String,
    thermal_profile_choices: Vec<String>,
    fan_mode: FanMode,
    rgb_mode: RgbMode,
    rgb_brightness: u8,
    fx_speed: u8,
    smart_battery_saver: bool,
    battery_limiter: bool,
    battery_calibration: bool,
    lcd_overdrive: bool,
    boot_animation: bool,
    usb_charging: u8,
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

fn bool_label(value: bool) -> &'static str {
    if value { "Enabled" } else { "Disabled" }
}

fn cycle_usb_threshold(current: u8) -> u8 {
    match current {
        0 => 10,
        10 => 20,
        20 => 30,
        _ => 0,
    }
}

fn next_thermal_profile(current: &str, choices: &[String]) -> Option<String> {
    if choices.is_empty() {
        return None;
    }

    if let Some(index) = choices.iter().position(|profile| profile == current) {
        return Some(choices[(index + 1) % choices.len()].clone());
    }

    choices.first().cloned()
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
        thermal_profile: "unknown".to_string(),
        thermal_profile_choices: Vec::new(),
        fan_mode: FanMode::Auto,
        rgb_mode: RgbMode::Solid(ProfessionalColor::ArchCyan),
        rgb_brightness: 70,
        fx_speed: 50,
        smart_battery_saver: false,
        battery_limiter: false,
        battery_calibration: false,
        lcd_overdrive: false,
        boot_animation: true,
        usb_charging: 0,
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
    let draw_tick = Duration::from_millis(120);
    let poll_tick = Duration::from_millis(900);
    let mut last_draw_tick = Instant::now();
    let mut last_poll_tick = Instant::now();

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

        let timeout = draw_tick
            .checked_sub(last_draw_tick.elapsed())
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
                KeyCode::Char('p') => {
                    if let Some(next) =
                        next_thermal_profile(&app.thermal_profile, &app.thermal_profile_choices)
                    {
                        app.last_response = send_command(Command::SetThermalProfile(next)).await;
                    } else {
                        app.last_response =
                            "✗ No thermal profile choices detected on this system".to_string();
                    }
                }
                KeyCode::Char('u') => {
                    let next = cycle_usb_threshold(app.usb_charging);
                    app.last_response = send_command(Command::SetUsbCharging(next)).await
                }
                KeyCode::Char('c') => {
                    app.last_response =
                        send_command(Command::SetBatteryCalibration(!app.battery_calibration)).await
                }
                KeyCode::Char('o') => {
                    app.last_response = send_command(Command::SetLcdOverdrive(!app.lcd_overdrive)).await
                }
                KeyCode::Char('m') => {
                    app.last_response = send_command(Command::SetBootAnimation(!app.boot_animation)).await
                }
                _ => {}
            }
        }

        if last_poll_tick.elapsed() >= poll_tick {
            sync_status(app).await;
            last_poll_tick = Instant::now();
        }

        if last_draw_tick.elapsed() >= draw_tick {
            last_draw_tick = Instant::now();
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
            Constraint::Length(12),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(11),
        ])
        .split(area);

    let status_lines = vec![
        Line::from(vec![
            Span::styled("Thermal: ", Style::default().fg(Color::Gray)),
            Span::styled(&app.thermal_profile, Style::default().fg(Color::White)),
        ]),
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
            Span::styled(bool_label(app.smart_battery_saver), Style::default().fg(if app.smart_battery_saver {
                Color::Green
            } else {
                Color::DarkGray
            })),
        ]),
        Line::from(vec![
            Span::styled("Batt Limit: ", Style::default().fg(Color::Gray)),
            Span::styled(bool_label(app.battery_limiter), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Calibration: ", Style::default().fg(Color::Gray)),
            Span::styled(bool_label(app.battery_calibration), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("LCD Override: ", Style::default().fg(Color::Gray)),
            Span::styled(bool_label(app.lcd_overdrive), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Boot Sound: ", Style::default().fg(Color::Gray)),
            Span::styled(bool_label(app.boot_animation), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("USB Charge: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}%", app.usb_charging), Style::default().fg(Color::White)),
        ]),
    ];

    let status_block = Paragraph::new(status_lines)
        .block(
            Block::default()
                .title(" Live Device State ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .alignment(Alignment::Left);
    frame.render_widget(status_block, chunks[0]);

    let thermal_choices = if app.thermal_profile_choices.is_empty() {
        "Unavailable".to_string()
    } else {
        app.thermal_profile_choices.join(" | ")
    };

    let thermal_controls = Paragraph::new(vec![
        Line::from(Span::styled(
            "Thermal Profile (ACPI Platform Profile)",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(" Current: {}", app.thermal_profile)),
        Line::from(format!(" Choices: {}", thermal_choices)),
        Line::from(" [p] Cycle to next supported profile"),
    ])
    .block(
        Block::default()
            .title(" Thermal ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    frame.render_widget(thermal_controls, chunks[1]);

    let fan_power = Paragraph::new(vec![
        Line::from(Span::styled(
            "Fan Controls (Independent from Thermal Profile)",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(" [a] Auto fan curve  [b] 50/50  [t] 100/100"),
        Line::from(" Fan mode writes to: predator_sense/fan_speed"),
    ])
    .block(
        Block::default()
            .title(" Fans ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    frame.render_widget(fan_power, chunks[2]);

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
            "Power / System",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(" [l] Battery Limiter  [c] Calibration"),
        Line::from(" [s] Toggle 30s Smart Saver"),
        Line::from(" [o] LCD Override  [m] Boot Animation Sound"),
        Line::from(" [u] Cycle USB Charging 0/10/20/30"),
        Line::from(" [q] Quit"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );

    frame.render_widget(illumination_system, chunks[3]);
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
        thermal_profile,
        thermal_profile_choices,
        fan_mode,
        active_rgb_mode,
        rgb_brightness,
        fx_speed,
        smart_battery_saver,
        battery_limiter,
        battery_calibration,
        lcd_overdrive,
        boot_animation,
        usb_charging,
    }) = response
    {
        app.cpu_temp = cpu_temp;
        app.gpu_temp = gpu_temp;
        app.cpu_fan = cpu_fan_percent;
        app.gpu_fan = gpu_fan_percent;
        app.thermal_profile = thermal_profile;
        app.thermal_profile_choices = thermal_profile_choices;
        app.fan_mode = fan_mode;
        app.rgb_mode = active_rgb_mode;
        app.rgb_brightness = rgb_brightness;
        app.fx_speed = fx_speed;
        app.smart_battery_saver = smart_battery_saver;
        app.battery_limiter = battery_limiter;
        app.battery_calibration = battery_calibration;
        app.lcd_overdrive = lcd_overdrive;
        app.boot_animation = boot_animation;
        app.usb_charging = usb_charging;
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
