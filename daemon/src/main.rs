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
const BRIGHTNESS_STEP: u8 = 10;

const FAN_CURVE: &[(u8, u8)] = &[(40, 20), (55, 40), (70, 65), (85, 100)];

fn calculate_fan_speed(current_temp: u8, curve: &[(u8, u8)]) -> u8 {
    if curve.is_empty() {
        return 0;
    }

    let first = curve[0];
    if current_temp <= first.0 {
        return first.1;
    }

    if let Some(&(last_temp, last_fan)) = curve.last()
        && current_temp >= last_temp
    {
        return last_fan;
    }

    for window in curve.windows(2) {
        let (t1, f1) = window[0];
        let (t2, f2) = window[1];
        if current_temp >= t1 && current_temp <= t2 {
            let temp_range = (t2 - t1) as f32;
            if temp_range <= f32::EPSILON {
                return f2;
            }
            let fan_range = (f2 - f1) as f32;
            return (f1 as f32 + (((current_temp - t1) as f32 / temp_range) * fan_range)) as u8;
        }
    }

    first.1
}

#[tokio::main]
async fn main() {
    println!("Starting Arch-Sense daemon...");
    let _ = fs::remove_file(SOCKET_PATH);

    let initial_config = DaemonConfig::load();
    let shared_config = Arc::new(Mutex::new(initial_config.clone()));

    println!("Applying persisted hardware state...");
    if let Err(err) = apply_saved_state(&initial_config).await {
        eprintln!("Failed while applying startup state: {err}");
    }

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

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("Failed to bind socket at {SOCKET_PATH}: {err}");
            return;
        }
    };

    if let Err(err) = fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o777)) {
        eprintln!("Failed to set socket permissions: {err}");
        return;
    }

    println!("Listening on {SOCKET_PATH}");

    loop {
        match listener.accept().await {
            Ok((mut socket, _addr)) => {
                let config_for_socket = Arc::clone(&shared_config);

                tokio::spawn(async move {
                    let mut buffer = vec![0; 2048];
                    let bytes_read = match socket.read(&mut buffer).await {
                        Ok(n) => n,
                        Err(err) => {
                            let _ =
                                write_response(&mut socket, Response::Error(format!("Read failed: {err}")))
                                    .await;
                            return;
                        }
                    };

                    if bytes_read == 0 {
                        return;
                    }

                    let request: Result<Command, _> = serde_json::from_slice(&buffer[..bytes_read]);
                    let response = match request {
                        Ok(command) => handle_command(command, &config_for_socket).await,
                        Err(err) => Response::Error(format!("Invalid command payload: {err}")),
                    };

                    let _ = write_response(&mut socket, response).await;
                });
            }
            Err(err) => eprintln!("Accept error: {err}"),
        }
    }
}

async fn apply_saved_state(config: &DaemonConfig) -> Result<(), String> {
    HardwareInterface::set_battery_limiter(config.battery_limiter).await?;
    HardwareInterface::set_lcd_overdrive(config.lcd_overdrive).await?;
    HardwareInterface::set_boot_animation(config.boot_animation).await?;
    HardwareInterface::set_backlight_timeout(config.smart_battery_saver).await?;
    HardwareInterface::set_usb_charging(config.usb_charging).await?;

    KeyboardInterface::apply_mode(&config.rgb_mode, config.rgb_brightness, config.fx_speed)
}

async fn handle_command(command: Command, shared_config: &Arc<Mutex<DaemonConfig>>) -> Response {
    match command {
        Command::GetHardwareStatus => {
            let cfg = {
                let cfg = shared_config.lock().await;
                cfg.clone()
            };

            let (cpu_fan, gpu_fan) = HardwareInterface::get_fan_speed().await.unwrap_or((0, 0));
            let cpu_temp = HardwareInterface::get_cpu_temp().await.unwrap_or(0);
            let gpu_temp = HardwareInterface::get_gpu_temp().await.unwrap_or(0);

            Response::HardwareStatus {
                cpu_temp,
                gpu_temp,
                cpu_fan_percent: cpu_fan,
                gpu_fan_percent: gpu_fan,
                fan_mode: cfg.fan_mode,
                active_rgb_mode: cfg.rgb_mode,
                rgb_brightness: cfg.rgb_brightness,
                fx_speed: cfg.fx_speed,
                smart_battery_saver: cfg.smart_battery_saver,
                battery_limiter: cfg.battery_limiter,
            }
        }
        Command::SetFanMode(new_mode) => match HardwareInterface::set_fan_mode(new_mode.clone()).await {
            Ok(_) => {
                persist_config(shared_config, |cfg| cfg.fan_mode = new_mode.clone()).await;
                Response::Ack(format!("Fan mode set to {:?}", new_mode))
            }
            Err(err) => Response::Error(err),
        },
        Command::SetBatteryLimiter(enable) => {
            match HardwareInterface::set_battery_limiter(enable).await {
                Ok(_) => {
                    persist_config(shared_config, |cfg| cfg.battery_limiter = enable).await;
                    Response::Ack(format!("Battery limiter set to {enable}"))
                }
                Err(err) => Response::Error(err),
            }
        }
        Command::SetBatteryCalibration(enable) => {
            match HardwareInterface::set_battery_calibration(enable).await {
                Ok(_) => Response::Ack(format!("Battery calibration set to {enable}")),
                Err(err) => Response::Error(err),
            }
        }
        Command::SetRgbMode(mode) => {
            let snapshot = {
                let cfg = shared_config.lock().await;
                (cfg.rgb_brightness, cfg.fx_speed)
            };

            match KeyboardInterface::apply_mode(&mode, snapshot.0, snapshot.1) {
                Ok(_) => {
                    persist_config(shared_config, |cfg| cfg.rgb_mode = mode.clone()).await;
                    Response::Ack(format!("RGB mode set to {:?}", mode))
                }
                Err(err) => Response::Error(err),
            }
        }
        Command::IncreaseRgbBrightness => {
            update_brightness(shared_config, true).await
        }
        Command::DecreaseRgbBrightness => {
            update_brightness(shared_config, false).await
        }
        Command::ToggleSmartBatterySaver => {
            let target = {
                let cfg = shared_config.lock().await;
                !cfg.smart_battery_saver
            };

            match HardwareInterface::set_backlight_timeout(target).await {
                Ok(_) => {
                    persist_config(shared_config, |cfg| cfg.smart_battery_saver = target).await;
                    Response::Ack(format!(
                        "30-second Smart Battery Saver {}",
                        if target { "enabled" } else { "disabled" }
                    ))
                }
                Err(err) => Response::Error(err),
            }
        }
        Command::SetLcdOverdrive(enable) => match HardwareInterface::set_lcd_overdrive(enable).await {
            Ok(_) => {
                persist_config(shared_config, |cfg| cfg.lcd_overdrive = enable).await;
                Response::Ack(format!("LCD overdrive set to {enable}"))
            }
            Err(err) => Response::Error(err),
        },
        Command::SetBootAnimation(enable) => match HardwareInterface::set_boot_animation(enable).await {
            Ok(_) => {
                persist_config(shared_config, |cfg| cfg.boot_animation = enable).await;
                Response::Ack(format!("Boot animation set to {enable}"))
            }
            Err(err) => Response::Error(err),
        },
        Command::SetUsbCharging(threshold) => {
            match HardwareInterface::set_usb_charging(threshold).await {
                Ok(_) => {
                    persist_config(shared_config, |cfg| cfg.usb_charging = threshold).await;
                    Response::Ack(format!("USB charging threshold set to {threshold}%"))
                }
                Err(err) => Response::Error(err),
            }
        }
    }
}

async fn update_brightness(shared_config: &Arc<Mutex<DaemonConfig>>, increase: bool) -> Response {
    let (mode, current, fx_speed) = {
        let cfg = shared_config.lock().await;
        (cfg.rgb_mode.clone(), cfg.rgb_brightness, cfg.fx_speed)
    };

    let target = if increase {
        current.saturating_add(BRIGHTNESS_STEP).min(100)
    } else {
        current.saturating_sub(BRIGHTNESS_STEP)
    };

    if target == current {
        return Response::Ack(format!("RGB brightness remains at {current}%"));
    }

    match KeyboardInterface::apply_mode(&mode, target, fx_speed) {
        Ok(_) => {
            persist_config(shared_config, |cfg| cfg.rgb_brightness = target).await;
            Response::Ack(format!("RGB brightness set to {target}%"))
        }
        Err(err) => Response::Error(err),
    }
}

async fn persist_config<F>(shared_config: &Arc<Mutex<DaemonConfig>>, mutate: F)
where
    F: FnOnce(&mut DaemonConfig),
{
    let mut cfg = shared_config.lock().await;
    mutate(&mut cfg);
    cfg.save();
}

async fn write_response(
    socket: &mut tokio::net::UnixStream,
    response: Response,
) -> Result<(), std::io::Error> {
    let response_bytes = match serde_json::to_vec(&response) {
        Ok(bytes) => bytes,
        Err(err) => {
            let fallback = Response::Error(format!("Response serialization failed: {err}"));
            match serde_json::to_vec(&fallback) {
                Ok(bytes) => bytes,
                Err(_) => return Ok(()),
            }
        }
    };

    socket.write_all(&response_bytes).await
}
