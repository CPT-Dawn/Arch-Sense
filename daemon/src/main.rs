mod config;
mod hardware;
mod keyboard;

use config::DaemonConfig;
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

    // 1. 📂 LOAD PERSISTENT STATE
    let initial_config = DaemonConfig::load();
    let shared_config = Arc::new(Mutex::new(initial_config.clone()));

    // 2. ⚡ APPLY HARDWARE STATE ON BOOT
    println!("⚙️ Applying saved hardware configuration...");
    let _ = HardwareInterface::set_battery_limiter(initial_config.battery_limiter).await;
    let _ = HardwareInterface::set_lcd_overdrive(initial_config.lcd_overdrive).await;
    let _ = HardwareInterface::set_boot_animation(initial_config.boot_animation).await;
    let _ = HardwareInterface::set_backlight_timeout(initial_config.backlight_timeout).await;
    let _ = HardwareInterface::set_usb_charging(initial_config.usb_charging).await;

    if let Some((r, g, b)) = initial_config.keyboard_color {
        let _ = KeyboardInterface::set_global_color(r, g, b);
    } else if let Some(anim) = initial_config.keyboard_animation {
        let _ = KeyboardInterface::set_animation(&anim);
    }

    // 3. 🚀 THE BACKGROUND WORKER (Fan Curve Loop)
    let config_for_worker = Arc::clone(&shared_config);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(2)).await;

            let mode = {
                let lock = config_for_worker.lock().await;
                lock.fan_mode.clone()
            };

            if let FanMode::Auto = mode
                && let Ok(temp) = HardwareInterface::get_cpu_temp().await
            {
                let target_speed = calculate_fan_speed(temp, FAN_CURVE);
                let _ =
                    HardwareInterface::set_fan_mode(FanMode::Custom(target_speed, target_speed))
                        .await;
            }
        }
    });

    // 4. 🎧 THE SOCKET LISTENER
    let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind socket.");
    fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o777))
        .expect("Failed to set socket permissions");
    println!("🎧 Listening for UI commands on {}...", SOCKET_PATH);

    loop {
        match listener.accept().await {
            Ok((mut socket, _addr)) => {
                let config_for_socket = Arc::clone(&shared_config);

                tokio::spawn(async move {
                    let mut buffer = vec![0; 1024];
                    if let Ok(bytes_read) = socket.read(&mut buffer).await {
                        if bytes_read == 0 {
                            return;
                        }
                        let request: Result<Command, _> =
                            serde_json::from_slice(&buffer[..bytes_read]);

                        // Lock the state manager!
                        let mut cfg = config_for_socket.lock().await;

                        let response = match request {
                            // UI wants live stats
                            Ok(Command::GetHardwareStatus) => {
                                let (cpu_fan, gpu_fan) =
                                    HardwareInterface::get_fan_speed().await.unwrap_or((0, 0));
                                let cpu_temp = HardwareInterface::get_cpu_temp().await.unwrap_or(0);
                                let gpu_temp = HardwareInterface::get_gpu_temp().await.unwrap_or(0);

                                Response::HardwareStatus {
                                    cpu_temp,
                                    gpu_temp,
                                    cpu_fan_percent: cpu_fan,
                                    gpu_fan_percent: gpu_fan,
                                    active_mode: format!("{:?}", cfg.fan_mode), // Read directly from config
                                }
                            }

                            // 🌬️ FANS & POWER
                            Ok(Command::SetFanMode(new_mode)) => {
                                cfg.fan_mode = new_mode.clone();
                                cfg.save();

                                if !matches!(new_mode, FanMode::Auto) {
                                    let _ = HardwareInterface::set_fan_mode(new_mode.clone()).await;
                                }
                                Response::Ack(format!("Mode changed to {:?}", new_mode))
                            }
                            Ok(Command::SetBatteryLimiter(enable)) => {
                                cfg.battery_limiter = enable;
                                cfg.save();
                                let _ = HardwareInterface::set_battery_limiter(enable).await;
                                Response::Ack(format!("Battery limiter set to {}", enable))
                            }
                            Ok(Command::SetBatteryCalibration(enable)) => {
                                match HardwareInterface::set_battery_calibration(enable).await {
                                    Ok(_) => Response::Ack(format!(
                                        "Battery Calibration set to {}",
                                        enable
                                    )),
                                    Err(e) => Response::Error(e),
                                }
                            }

                            // 💡 RGB CONTROLS
                            Ok(Command::SetKeyboardColor(r, g, b)) => {
                                cfg.keyboard_color = Some((r, g, b));
                                cfg.keyboard_animation = None;
                                cfg.save();

                                match KeyboardInterface::set_global_color(r, g, b) {
                                    Ok(_) => Response::Ack(format!(
                                        "Keyboard color set to RGB({},{},{})",
                                        r, g, b
                                    )),
                                    Err(e) => Response::Error(e),
                                }
                            }
                            Ok(Command::SetKeyboardAnimation(effect)) => {
                                cfg.keyboard_animation = Some(effect.clone());
                                cfg.keyboard_color = None;
                                cfg.save();

                                match KeyboardInterface::set_animation(&effect) {
                                    Ok(_) => Response::Ack(format!(
                                        "Keyboard animation set to '{}'",
                                        effect
                                    )),
                                    Err(e) => Response::Error(e),
                                }
                            }

                            // ⚙️ SYSTEM TOGGLES
                            Ok(Command::SetLcdOverdrive(enable)) => {
                                cfg.lcd_overdrive = enable;
                                cfg.save();

                                match HardwareInterface::set_lcd_overdrive(enable).await {
                                    Ok(_) => {
                                        Response::Ack(format!("LCD Overdrive set to {}", enable))
                                    }
                                    Err(e) => Response::Error(e),
                                }
                            }
                            Ok(Command::SetBootAnimation(enable)) => {
                                cfg.boot_animation = enable;
                                cfg.save();

                                match HardwareInterface::set_boot_animation(enable).await {
                                    Ok(_) => {
                                        Response::Ack(format!("Boot Animation set to {}", enable))
                                    }
                                    Err(e) => Response::Error(e),
                                }
                            }
                            Ok(Command::SetBacklightTimeout(enable)) => {
                                cfg.backlight_timeout = enable;
                                cfg.save();

                                match HardwareInterface::set_backlight_timeout(enable).await {
                                    Ok(_) => {
                                        Response::Ack(format!("Keyboard Timeout set to {}", enable))
                                    }
                                    Err(e) => Response::Error(e),
                                }
                            }
                            Ok(Command::SetUsbCharging(threshold)) => {
                                cfg.usb_charging = threshold;
                                cfg.save();

                                match HardwareInterface::set_usb_charging(threshold).await {
                                    Ok(_) => Response::Ack(format!(
                                        "USB Charging threshold set to {}%",
                                        threshold
                                    )),
                                    Err(e) => Response::Error(e),
                                }
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
