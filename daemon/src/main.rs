mod hardware;
mod keyboard;

use hardware::HardwareInterface;
use keyboard::KeyboardInterface;
use shared::{Command, FanMode, Response};
use std::os::unix::fs::PermissionsExt;
use std::{fs, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    sync::Mutex,
    time::{Duration, sleep},
};

const SOCKET_PATH: &str = "/tmp/arch-sense.sock";

// (Temperature °C, Fan Speed %)
const FAN_CURVE: &[(u8, u8)] = &[
    (40, 20),  // At 40°C, fans run quietly at 20%
    (55, 40),  // At 55°C, fans ramp up to 40%
    (70, 65),  // At 70°C, fans ramp up to 65%
    (85, 100), // At 85°C+, full 100% blast to save the laptop
];

// Linear Interpolation function
fn calculate_fan_speed(current_temp: u8, curve: &[(u8, u8)]) -> u8 {
    if current_temp <= curve[0].0 {
        return curve[0].1;
    }
    if current_temp >= curve.last().unwrap().0 {
        return curve.last().unwrap().1;
    }
    for i in 0..(curve.len() - 1) {
        let (t1, f1) = curve[i];
        let (t2, f2) = curve[i + 1];
        if current_temp >= t1 && current_temp <= t2 {
            let temp_range = (t2 - t1) as f32;
            let fan_range = (f2 - f1) as f32;
            return (f1 as f32 + (((current_temp - t1) as f32 / temp_range) * fan_range)) as u8;
        }
    }
    0
}

#[tokio::main]
async fn main() {
    println!("🔥 Starting Arch-Sense Background Daemon...");
    let _ = fs::remove_file(SOCKET_PATH);

    // 1. Shared State: The Daemon starts in Auto (Custom Curve) mode
    let current_mode = Arc::new(Mutex::new(FanMode::Auto));

    // 2. 🚀 THE BACKGROUND WORKER (Fan Curve Loop)
    let mode_for_worker = Arc::clone(&current_mode);
    tokio::spawn(async move {
        loop {
            // Check the hardware every 2 seconds
            sleep(Duration::from_secs(2)).await;

            let mode = {
                let lock = mode_for_worker.lock().await;
                lock.clone()
            };

            // If the user selected 'Auto', our Rust daemon takes over and applies the curve!
            if let FanMode::Auto = mode
                && let Ok(temp) = HardwareInterface::get_cpu_temp().await
            {
                let target_speed = calculate_fan_speed(temp, FAN_CURVE);

                // Secretly apply the custom percentage to the hardware
                let _ =
                    HardwareInterface::set_fan_mode(FanMode::Custom(target_speed, target_speed))
                        .await;
            }
        }
    });

    // 3. 🎧 THE SOCKET LISTENER (UI Communications)
    let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind socket.");

    // 🔓 THE FIX: Make the root-owned socket writable by your normal user!
    fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o777))
        .expect("Failed to set socket permissions");

    println!("🎧 Listening for UI commands on {}...", SOCKET_PATH);

    loop {
        match listener.accept().await {
            Ok((mut socket, _addr)) => {
                let mode_for_socket = Arc::clone(&current_mode);

                tokio::spawn(async move {
                    let mut buffer = vec![0; 1024];
                    if let Ok(bytes_read) = socket.read(&mut buffer).await {
                        if bytes_read == 0 {
                            return;
                        }
                        let request: Result<Command, _> =
                            serde_json::from_slice(&buffer[..bytes_read]);

                        let response = match request {
                            // UI wants live stats
                            Ok(Command::GetHardwareStatus) => {
                                let (cpu_fan, gpu_fan) =
                                    HardwareInterface::get_fan_speed().await.unwrap_or((0, 0));
                                let cpu_temp = HardwareInterface::get_cpu_temp().await.unwrap_or(0);

                                // Tell the UI what mode we are actively enforcing
                                let active_mode_str = {
                                    let lock = mode_for_socket.lock().await;
                                    format!("{:?}", *lock)
                                };

                                Response::HardwareStatus {
                                    cpu_temp,
                                    gpu_temp: 40, // Keeping GPU temp hardcoded until we link nvidia-smi
                                    cpu_fan_percent: cpu_fan,
                                    gpu_fan_percent: gpu_fan,
                                    active_mode: active_mode_str,
                                }
                            }

                            // UI wants to change the keyboard color!
                            Ok(Command::SetKeyboardColor(r, g, b)) => {
                                match KeyboardInterface::set_global_color(r, g, b) {
                                    Ok(_) => Response::Ack(format!(
                                        "Keyboard color set to RGB({},{},{})",
                                        r, g, b
                                    )),
                                    Err(e) => Response::Error(e),
                                }
                            }
                            // UI wants to change the mode
                            Ok(Command::SetFanMode(new_mode)) => {
                                // 1. Update our shared state so the background worker knows
                                {
                                    let mut lock = mode_for_socket.lock().await;
                                    *lock = new_mode.clone();
                                }

                                // 2. If it's NOT Auto, apply it to the hardware immediately
                                if !matches!(new_mode, FanMode::Auto) {
                                    let _ = HardwareInterface::set_fan_mode(new_mode.clone()).await;
                                }

                                Response::Ack(format!("Mode changed to {:?}", new_mode))
                            }

                            Ok(Command::SetBatteryLimiter(enable)) => {
                                let _ = HardwareInterface::set_battery_limiter(enable).await;
                                Response::Ack(format!("Battery limiter set to {}", enable))
                            }
                            _ => Response::Error("Unknown command".to_string()),
                        };

                        let response_bytes = serde_json::to_vec(&response).unwrap();
                        let _ = socket.write_all(&response_bytes).await;
                    }
                });
            }
            Err(e) => eprintln!("❌ Error accepting connection: {}", e),
        }
    }
}
