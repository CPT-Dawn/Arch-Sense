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

struct App {
    last_response: String,
    cpu_fan: u8,
    gpu_fan: u8,
    cpu_temp: u8,
    gpu_temp: u8,
    active_mode: String,
}

// 🎨 Helper function to dynamically color temperatures
fn get_temp_color(temp: u8) -> Color {
    match temp {
        0..=60 => Color::Green,
        61..=80 => Color::Yellow,
        _ => Color::Red,
    }
}

// 🎨 Helper function to dynamically color fan speeds
fn get_fan_color(speed: u8) -> Color {
    match speed {
        0..=30 => Color::Cyan,
        31..=70 => Color::Blue,
        _ => Color::Magenta,
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
        last_response: "Connecting to Daemon...".to_string(),
        cpu_fan: 0,
        gpu_fan: 0,
        cpu_temp: 0,
        gpu_temp: 0,
        active_mode: "Unknown".to_string(),
    };

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
    let tick_rate = Duration::from_millis(500); // Polling twice a second for smoother UI
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            // ==========================================
            // 📐 MASTER LAYOUT
            // ==========================================
            let size = f.size();
            let master_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // Banner
                    Constraint::Min(10),   // Main Grid
                    Constraint::Length(3), // Status Footer
                ])
                .split(size);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50), // Left: Telemetry
                    Constraint::Percentage(50), // Right: Controls
                ])
                .split(master_chunks[1]);

            let telemetry_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // CPU Temp
                    Constraint::Length(3), // CPU Fan
                    Constraint::Length(3), // GPU Temp
                    Constraint::Length(3), // GPU Fan
                    Constraint::Min(0),
                ])
                .split(main_chunks[0]);

            let controls_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(60), // Hotkeys
                    Constraint::Percentage(40), // System Info
                ])
                .split(main_chunks[1]);

            // ==========================================
            // 🖼️ WIDGETS
            // ==========================================

            // 1. BANNER
            let banner = Paragraph::new(Line::from(vec![
                Span::styled(
                    " 🐉 ARCH-SENSE ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled(
                    "Predator Control Center",
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Center);
            f.render_widget(banner, master_chunks[0]);

            // 2. CPU TEMP GAUGE
            let cpu_temp_color = get_temp_color(app.cpu_temp);
            let cpu_temp_gauge = Gauge::default()
                .block(
                    Block::default()
                        .title(" CPU Temperature (°C) ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(Style::default().fg(cpu_temp_color).bg(Color::Black))
                .percent(app.cpu_temp.min(100) as u16)
                .label(format!("{}°C", app.cpu_temp));
            f.render_widget(cpu_temp_gauge, telemetry_chunks[0]);

            // 3. CPU FAN GAUGE
            let cpu_fan_gauge = Gauge::default()
                .block(
                    Block::default()
                        .title(" CPU Fan Speed (%) ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(
                    Style::default()
                        .fg(get_fan_color(app.cpu_fan))
                        .bg(Color::Black),
                )
                .percent(app.cpu_fan.min(100) as u16)
                .label(format!("{}%", app.cpu_fan));
            f.render_widget(cpu_fan_gauge, telemetry_chunks[1]);

            // 4. GPU TEMP GAUGE
            let gpu_temp_color = get_temp_color(app.gpu_temp);
            let gpu_temp_gauge = Gauge::default()
                .block(
                    Block::default()
                        .title(" GPU Temperature (°C) ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(Style::default().fg(gpu_temp_color).bg(Color::Black))
                .percent(app.gpu_temp.min(100) as u16)
                .label(format!("{}°C", app.gpu_temp));
            f.render_widget(gpu_temp_gauge, telemetry_chunks[2]);

            // 5. GPU FAN GAUGE
            let gpu_fan_gauge = Gauge::default()
                .block(
                    Block::default()
                        .title(" GPU Fan Speed (%) ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .gauge_style(
                    Style::default()
                        .fg(get_fan_color(app.gpu_fan))
                        .bg(Color::Black),
                )
                .percent(app.gpu_fan.min(100) as u16)
                .label(format!("{}%", app.gpu_fan));
            f.render_widget(gpu_fan_gauge, telemetry_chunks[3]);

            // 6. HOTKEYS MENU
            let hotkeys_text = vec![
                Line::from(Span::styled(
                    "🌬️ FAN CONTROL",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(" [a] Auto | [b] Balanced | [t] Turbo"),
                Line::from(""),
                Line::from(Span::styled(
                    "🔋 POWER & BATTERY",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(" [l] Toggle 80% Limit | [c] Calibrate"),
                Line::from(" [u] Cycle USB Charging (0/10/20/30)"),
                Line::from(""),
                Line::from(Span::styled(
                    "💡 RGB & FX",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(" [1] Red | [2] Grn | [3] Blu | [4] Wht | [5] Pnk"),
                Line::from(" [7] Neon | [8] Wave | [9] Breath"),
                Line::from(" [k] Toggle 30s Timeout"),
                Line::from(""),
                Line::from(Span::styled(
                    "⚙️ SYSTEM",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(" [o] Toggle 3ms LCD Overdrive"),
                Line::from(" [m] Toggle Boot Sound"),
                Line::from(""),
                Line::from(Span::styled(
                    " [q] Quit UI ",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let hotkeys_block = Paragraph::new(hotkeys_text).block(
                Block::default()
                    .title(" Command Matrix ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
            f.render_widget(hotkeys_block, controls_chunks[0]);

            // 7. SYSTEM INFO BOX
            let sys_info = vec![Line::from(vec![
                Span::raw("Active Fan Mode: "),
                Span::styled(&app.active_mode, Style::default().fg(Color::Green)),
            ])];
            let sys_block = Paragraph::new(sys_info).block(
                Block::default()
                    .title(" System Status ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
            f.render_widget(sys_block, controls_chunks[1]);

            // 8. DAEMON RESPONSE FOOTER
            let footer = Paragraph::new(app.last_response.clone())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Center);
            f.render_widget(footer, master_chunks[2]);
        })?;

        // ==========================================
        // ⌨️ INPUT HANDLING
        // ==========================================
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
        {
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
                KeyCode::Char('7') => {
                    app.last_response =
                        send_command(Command::SetKeyboardAnimation("neon".to_string())).await
                }
                KeyCode::Char('8') => {
                    app.last_response =
                        send_command(Command::SetKeyboardAnimation("wave".to_string())).await
                }
                KeyCode::Char('9') => {
                    app.last_response =
                        send_command(Command::SetKeyboardAnimation("breath".to_string())).await
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
                    app.last_response = send_command(Command::SetBatteryLimiter(true)).await
                }
                KeyCode::Char('c') => {
                    app.last_response = send_command(Command::SetBatteryCalibration(true)).await
                }
                KeyCode::Char('o') => {
                    app.last_response = send_command(Command::SetLcdOverdrive(true)).await
                }
                KeyCode::Char('m') => {
                    app.last_response = send_command(Command::SetBootAnimation(false)).await
                }
                KeyCode::Char('k') => {
                    app.last_response = send_command(Command::SetBacklightTimeout(true)).await
                }
                KeyCode::Char('u') => {
                    app.last_response = send_command(Command::SetUsbCharging(30)).await
                }
                _ => {}
            }
        }

        // ==========================================
        // 🔄 BACKGROUND POLLING
        // ==========================================
        if last_tick.elapsed() >= tick_rate {
            if let Ok(mut stream) = UnixStream::connect("/tmp/arch-sense.sock").await {
                let msg = serde_json::to_vec(&Command::GetHardwareStatus).unwrap();
                if stream.write_all(&msg).await.is_ok() {
                    let mut buf = vec![0; 1024];
                    if let Ok(n) = stream.read(&mut buf).await
                        && let Ok(Response::HardwareStatus {
                            cpu_temp,
                            gpu_temp,
                            cpu_fan_percent,
                            gpu_fan_percent,
                            active_mode,
                        }) = serde_json::from_slice(&buf[..n])
                    {
                        app.cpu_temp = cpu_temp;
                        app.gpu_temp = gpu_temp;
                        app.cpu_fan = cpu_fan_percent;
                        app.gpu_fan = gpu_fan_percent;
                        app.active_mode = active_mode;
                    }
                }
            }
            last_tick = Instant::now();
        }
    }
}

async fn send_command(cmd: Command) -> String {
    let mut stream = match UnixStream::connect("/tmp/arch-sense.sock").await {
        Ok(s) => s,
        Err(_) => return "❌ Could not connect! Is the Daemon running?".to_string(),
    };

    let msg = serde_json::to_vec(&cmd).unwrap();
    let _ = stream.write_all(&msg).await;

    let mut buf = vec![0; 1024];
    if let Ok(n) = stream.read(&mut buf).await {
        let res: Result<Response, _> = serde_json::from_slice(&buf[..n]);
        match res {
            Ok(Response::Ack(msg)) => format!("✅ {}", msg),
            Ok(Response::Error(err)) => format!("❌ {}", err),
            _ => "⚠️ Daemon sent an unknown response".to_string(),
        }
    } else {
        "⚠️ Failed to read reply from Daemon".to_string()
    }
}
