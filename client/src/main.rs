use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
};
use shared::{Command, FanMode, Response};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/arch-sense.sock";
const EFFECTS: [&str; 10] = [
    "neon",
    "wave",
    "breath",
    "rainbow",
    "reactive",
    "ripple",
    "starlight",
    "rain",
    "fire",
    "aurora",
];

struct App {
    last_response: String,
    cpu_fan: u8,
    gpu_fan: u8,
    cpu_temp: u8,
    gpu_temp: u8,
    active_mode: String,
    battery_limiter: bool,
    lcd_overdrive: bool,
    boot_animation: bool,
    backlight_timeout: bool,
    usb_charging: u8,
    keyboard_color: Option<(u8, u8, u8)>,
    keyboard_animation: Option<String>,
    keyboard_speed: u8,
    keyboard_brightness: u8,
    selected_effect_idx: usize,
}

fn get_temp_color(temp: u8) -> Color {
    match temp {
        0..=60 => Color::Green,
        61..=80 => Color::Yellow,
        _ => Color::Red,
    }
}

fn get_fan_color(speed: u8) -> Color {
    match speed {
        0..=30 => Color::Cyan,
        31..=70 => Color::Blue,
        _ => Color::Magenta,
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}

fn current_rgb_mode(app: &App) -> String {
    if let Some(effect) = &app.keyboard_animation {
        return format!("FX: {}", effect);
    }

    if let Some((r, g, b)) = app.keyboard_color {
        return format!("Static RGB({}, {}, {})", r, g, b);
    }

    "Unknown".to_string()
}

fn next_usb_threshold(current: u8) -> u8 {
    match current {
        0 => 10,
        10 => 20,
        20 => 30,
        _ => 0,
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
        active_mode: "Unknown".to_string(),
        battery_limiter: false,
        lcd_overdrive: false,
        boot_animation: false,
        backlight_timeout: false,
        usb_charging: 0,
        keyboard_color: Some((255, 0, 255)),
        keyboard_animation: None,
        keyboard_speed: 5,
        keyboard_brightness: 100,
        selected_effect_idx: 0,
    };

    let _ = refresh_status(&mut app).await;
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let tick_rate = Duration::from_millis(600);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(size);

            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(rows[1]);

            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(body[0]);

            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(48),
                    Constraint::Percentage(28),
                    Constraint::Percentage(24),
                ])
                .split(body[1]);

            let banner = Paragraph::new(Line::from(vec![
                Span::styled(
                    " ARCH-SENSE ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  •  "),
                Span::styled(
                    "Predator Control Dashboard",
                    Style::default().fg(Color::Gray),
                ),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Center);
            f.render_widget(banner, rows[0]);

            let cpu_temp = Gauge::default()
                .block(
                    Block::default()
                        .title(" CPU Temp ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(Style::default().fg(get_temp_color(app.cpu_temp)).bg(Color::Black))
                .percent(app.cpu_temp.min(100) as u16)
                .label(format!("{}°C", app.cpu_temp));
            f.render_widget(cpu_temp, left[0]);

            let cpu_fan = Gauge::default()
                .block(
                    Block::default()
                        .title(" CPU Fan ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(Style::default().fg(get_fan_color(app.cpu_fan)).bg(Color::Black))
                .percent(app.cpu_fan.min(100) as u16)
                .label(format!("{}%", app.cpu_fan));
            f.render_widget(cpu_fan, left[1]);

            let gpu_temp = Gauge::default()
                .block(
                    Block::default()
                        .title(" GPU Temp ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(Style::default().fg(get_temp_color(app.gpu_temp)).bg(Color::Black))
                .percent(app.gpu_temp.min(100) as u16)
                .label(format!("{}°C", app.gpu_temp));
            f.render_widget(gpu_temp, left[2]);

            let gpu_fan = Gauge::default()
                .block(
                    Block::default()
                        .title(" GPU Fan ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(Style::default().fg(get_fan_color(app.gpu_fan)).bg(Color::Black))
                .percent(app.gpu_fan.min(100) as u16)
                .label(format!("{}%", app.gpu_fan));
            f.render_widget(gpu_fan, left[3]);

            let controls = vec![
                Line::from(Span::styled(
                    "Fans",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )),
                Line::from(" [a] Auto  [b] Balanced  [t] Turbo"),
                Line::from(""),
                Line::from(Span::styled(
                    "RGB",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(" [1] Red [2] Green [3] Blue [4] White [5] Pink"),
                Line::from(" [n]/[p] Next/Prev effect  [x] Apply selected"),
                Line::from(" [+]/[-] RGB speed (1..10)"),
                Line::from(" [[]/[]] RGB brightness (0..100)"),
                Line::from(""),
                Line::from(Span::styled(
                    "System",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(" [l] Battery limit  [c] Battery calibration"),
                Line::from(" [o] LCD overdrive [m] Boot animation [k] Backlight timeout"),
                Line::from(" [u] USB charging cycle (0/10/20/30)  [q] Quit"),
            ];
            let controls_block = Paragraph::new(controls).block(
                Block::default()
                    .title(" Controls ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
            f.render_widget(controls_block, left[4]);

            let selected_effect = EFFECTS
                .get(app.selected_effect_idx)
                .copied()
                .unwrap_or("neon");
            let rgb_status = vec![
                Line::from(vec![
                    Span::raw("Current RGB Mode: "),
                    Span::styled(current_rgb_mode(app), Style::default().fg(Color::LightMagenta)),
                ]),
                Line::from(vec![
                    Span::raw("Selected Effect: "),
                    Span::styled(selected_effect, Style::default().fg(Color::Magenta)),
                ]),
                Line::from(vec![
                    Span::raw("RGB Speed: "),
                    Span::styled(app.keyboard_speed.to_string(), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::raw("RGB Brightness: "),
                    Span::styled(
                        format!("{}%", app.keyboard_brightness),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
            ];
            let rgb_panel = Paragraph::new(rgb_status).block(
                Block::default()
                    .title(" RGB State (Exact Values) ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
            f.render_widget(rgb_panel, right[0]);

            let system_status = vec![
                Line::from(vec![
                    Span::raw("Fan Mode: "),
                    Span::styled(&app.active_mode, Style::default().fg(Color::Green)),
                ]),
                Line::from(format!("Battery Limiter: {}", bool_label(app.battery_limiter))),
                Line::from(format!("LCD Overdrive: {}", bool_label(app.lcd_overdrive))),
                Line::from(format!("Boot Animation: {}", bool_label(app.boot_animation))),
                Line::from(format!(
                    "Backlight Timeout: {}",
                    bool_label(app.backlight_timeout)
                )),
                Line::from(format!("USB Charging Threshold: {}%", app.usb_charging)),
            ];
            let system_panel = Paragraph::new(system_status).block(
                Block::default()
                    .title(" System State (Exact Values) ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
            f.render_widget(system_panel, right[1]);

            let effects_panel = Paragraph::new(vec![
                Line::from(Span::styled(
                    "Popular RGB Designs",
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(EFFECTS.join(" • ")),
            ])
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .title(" Effects ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
            f.render_widget(effects_panel, right[2]);

            let footer = Paragraph::new(app.last_response.clone())
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                );
            f.render_widget(footer, rows[2]);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? && let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('1') => {
                    app.last_response = send_command(Command::SetKeyboardColor(255, 0, 0)).await
                }
                KeyCode::Char('2') => {
                    app.last_response = send_command(Command::SetKeyboardColor(0, 255, 0)).await
                }
                KeyCode::Char('3') => {
                    app.last_response = send_command(Command::SetKeyboardColor(0, 0, 255)).await
                }
                KeyCode::Char('4') => {
                    app.last_response = send_command(Command::SetKeyboardColor(255, 255, 255)).await
                }
                KeyCode::Char('5') => {
                    app.last_response = send_command(Command::SetKeyboardColor(255, 0, 255)).await
                }
                KeyCode::Char('n') => {
                    app.selected_effect_idx = (app.selected_effect_idx + 1) % EFFECTS.len();
                    app.last_response =
                        format!("Selected RGB effect: {}", EFFECTS[app.selected_effect_idx]);
                }
                KeyCode::Char('p') => {
                    app.selected_effect_idx = if app.selected_effect_idx == 0 {
                        EFFECTS.len() - 1
                    } else {
                        app.selected_effect_idx - 1
                    };
                    app.last_response =
                        format!("Selected RGB effect: {}", EFFECTS[app.selected_effect_idx]);
                }
                KeyCode::Char('x') => {
                    app.last_response = send_command(Command::SetKeyboardAnimation(
                        EFFECTS[app.selected_effect_idx].to_string(),
                    ))
                    .await;
                }
                KeyCode::Char('+') => {
                    if app.keyboard_speed < 10 {
                        app.last_response =
                            send_command(Command::SetKeyboardSpeed(app.keyboard_speed + 1)).await;
                    }
                }
                KeyCode::Char('-') => {
                    if app.keyboard_speed > 1 {
                        app.last_response =
                            send_command(Command::SetKeyboardSpeed(app.keyboard_speed - 1)).await;
                    }
                }
                KeyCode::Char(']') => {
                    if app.keyboard_brightness < 100 {
                        app.last_response = send_command(Command::SetKeyboardBrightness(
                            (app.keyboard_brightness + 5).min(100),
                        ))
                        .await;
                    }
                }
                KeyCode::Char('[') => {
                    if app.keyboard_brightness > 0 {
                        app.last_response = send_command(Command::SetKeyboardBrightness(
                            app.keyboard_brightness.saturating_sub(5),
                        ))
                        .await;
                    }
                }
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
                KeyCode::Char('c') => {
                    app.last_response = send_command(Command::SetBatteryCalibration(true)).await
                }
                KeyCode::Char('o') => {
                    app.last_response =
                        send_command(Command::SetLcdOverdrive(!app.lcd_overdrive)).await
                }
                KeyCode::Char('m') => {
                    app.last_response =
                        send_command(Command::SetBootAnimation(!app.boot_animation)).await
                }
                KeyCode::Char('k') => {
                    app.last_response =
                        send_command(Command::SetBacklightTimeout(!app.backlight_timeout)).await
                }
                KeyCode::Char('u') => {
                    app.last_response = send_command(Command::SetUsbCharging(next_usb_threshold(
                        app.usb_charging,
                    )))
                    .await
                }
                _ => {}
            }

            let _ = refresh_status(app).await;
        }

        if last_tick.elapsed() >= tick_rate {
            let _ = refresh_status(app).await;
            last_tick = Instant::now();
        }
    }
}

async fn refresh_status(app: &mut App) -> Result<(), String> {
    let response = send_command_raw(Command::GetHardwareStatus).await?;
    match response {
        Response::HardwareStatus {
            cpu_temp,
            gpu_temp,
            cpu_fan_percent,
            gpu_fan_percent,
            active_mode,
            battery_limiter,
            lcd_overdrive,
            boot_animation,
            backlight_timeout,
            usb_charging,
            keyboard_color,
            keyboard_animation,
            keyboard_speed,
            keyboard_brightness,
        } => {
            app.cpu_temp = cpu_temp;
            app.gpu_temp = gpu_temp;
            app.cpu_fan = cpu_fan_percent;
            app.gpu_fan = gpu_fan_percent;
            app.active_mode = active_mode;
            app.battery_limiter = battery_limiter;
            app.lcd_overdrive = lcd_overdrive;
            app.boot_animation = boot_animation;
            app.backlight_timeout = backlight_timeout;
            app.usb_charging = usb_charging;
            app.keyboard_color = keyboard_color;
            app.keyboard_animation = keyboard_animation.clone();
            app.keyboard_speed = keyboard_speed;
            app.keyboard_brightness = keyboard_brightness;

            if let Some(anim) = keyboard_animation
                && let Some(index) = EFFECTS.iter().position(|entry| *entry == anim)
            {
                app.selected_effect_idx = index;
            }

            Ok(())
        }
        Response::Ack(msg) => Err(msg),
        Response::Error(msg) => Err(msg),
    }
}

async fn send_command(cmd: Command) -> String {
    match send_command_raw(cmd).await {
        Ok(Response::Ack(msg)) => format!("✅ {}", msg),
        Ok(Response::Error(err)) => format!("❌ {}", err),
        Ok(Response::HardwareStatus { .. }) => "📊 Status refreshed".to_string(),
        Err(err) => format!("❌ {}", err),
    }
}

async fn send_command_raw(cmd: Command) -> Result<Response, String> {
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .await
        .map_err(|_| "Could not connect to daemon. Is it running?".to_string())?;

    let msg = serde_json::to_vec(&cmd).map_err(|e| format!("Serialize request failed: {}", e))?;
    stream
        .write_all(&msg)
        .await
        .map_err(|e| format!("Send request failed: {}", e))?;

    let mut buf = vec![0; 2048];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Read response failed: {}", e))?;

    if n == 0 {
        return Err("Daemon closed the connection without a response".to_string());
    }

    serde_json::from_slice(&buf[..n]).map_err(|e| format!("Invalid daemon response: {}", e))
}
