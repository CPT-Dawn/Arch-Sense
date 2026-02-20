use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize the App with zeroed out data
    let mut app = App {
        last_response: "Press a key to send a command to the Daemon.".to_string(),
        cpu_fan: 0,
        gpu_fan: 0,
        cpu_temp: 0,
        gpu_temp: 0,
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
    // Set up our background polling timer
    let tick_rate = Duration::from_secs(1);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
                .split(f.size());

            // The Controls Menu
            let controls_text = "
 🎮 Acer Predator Arch-Sense Control
 -----------------------------------
 [a] Set Fans to Auto
 [b] Set Fans to Balanced
 [t] Set Fans to Turbo (Max)
 
 [l] Toggle 80% Battery Limiter
 
 [q] Quit UI
            ";

            let controls_block = Paragraph::new(controls_text)
                .block(Block::default().title(" Controls ").borders(Borders::ALL));
            f.render_widget(controls_block, chunks[0]);

            // 4. The Live Telemetry Dashboard
            let live_stats = format!(
                "\n 🌡️  CPU Temp: {}°C  |  💨 CPU Fan: {}%\n 🌡️  GPU Temp: {}°C  |  💨 GPU Fan: {}%\n\n 📝 Daemon Status: {}",
                app.cpu_temp, app.cpu_fan, app.gpu_temp, app.gpu_fan, app.last_response
            );

            let status_block = Paragraph::new(live_stats)
                .style(Style::default().fg(Color::Cyan))
                .block(
                    Block::default()
                        .title(" Live Telemetry ")
                        .borders(Borders::ALL),
                );
            f.render_widget(status_block, chunks[1]);
        })?;

        // Calculate time remaining before we need to poll the daemon again
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        // Wait for keyboard input OR timeout for the next data tick
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('a') => {
                    app.last_response = send_command(Command::SetFanMode(FanMode::Auto)).await;
                }
                KeyCode::Char('b') => {
                    app.last_response = send_command(Command::SetFanMode(FanMode::Balanced)).await;
                }
                KeyCode::Char('t') => {
                    app.last_response = send_command(Command::SetFanMode(FanMode::Turbo)).await;
                }
                KeyCode::Char('l') => {
                    app.last_response = send_command(Command::SetBatteryLimiter(true)).await;
                }
                _ => {}
            }
        }

        // If 1 second has passed, secretly fetch new data from the Daemon!
        if last_tick.elapsed() >= tick_rate {
            if let Ok(mut stream) = UnixStream::connect("/tmp/arch-sense.sock").await {
                let msg = serde_json::to_vec(&Command::GetHardwareStatus).unwrap();
                if stream.write_all(&msg).await.is_ok() {
                    let mut buf = vec![0; 1024];
                    if let Ok(n) = stream.read(&mut buf).await {
                        // Parse the response and update the UI variables directly
                        if let Ok(Response::HardwareStatus {
                            cpu_temp,
                            gpu_temp,
                            cpu_fan_percent,
                            gpu_fan_percent,
                            ..
                        }) = serde_json::from_slice(&buf[..n])
                        {
                            app.cpu_temp = cpu_temp;
                            app.gpu_temp = gpu_temp;
                            app.cpu_fan = cpu_fan_percent;
                            app.gpu_fan = gpu_fan_percent;
                        }
                    }
                }
            }
            // Reset the clock for the next tick
            last_tick = Instant::now();
        }
    }
}

/// The Bridge: Connects to the Daemon socket, sends the command, and reads the reply
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
