mod hardware;

use hardware::HardwareInterface;
use shared::{Command, Response};
use std::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

const SOCKET_PATH: &str = "/tmp/arch-sense.sock";

#[tokio::main]
async fn main() {
    println!("🔥 Starting Arch-Sense Background Daemon...");

    let _ = fs::remove_file(SOCKET_PATH);

    let listener =
        UnixListener::bind(SOCKET_PATH).expect("Failed to bind socket. Is another daemon running?");

    println!("🎧 Listening for UI commands on {}...", SOCKET_PATH);

    loop {
        match listener.accept().await {
            Ok((mut socket, _addr)) => {
                tokio::spawn(async move {
                    let mut buffer = vec![0; 1024]; // A 1KB buffer for the incoming message

                    if let Ok(bytes_read) = socket.read(&mut buffer).await {
                        if bytes_read == 0 {
                            return;
                        }

                        let request: Result<Command, _> =
                            serde_json::from_slice(&buffer[..bytes_read]);

                        let response = match request {
                            Ok(Command::SetBatteryLimiter(enable)) => {
                                match HardwareInterface::set_battery_limiter(enable).await {
                                    Ok(_) => {
                                        Response::Ack(format!("Battery limiter set to {}", enable))
                                    }
                                    Err(e) => Response::Error(e),
                                }
                            }

                            Ok(Command::SetFanMode(mode)) => {
                                match HardwareInterface::set_fan_mode(mode.clone()).await {
                                    Ok(_) => Response::Ack(format!(
                                        "Fans successfully set to {:?}",
                                        mode
                                    )),
                                    Err(e) => Response::Error(e),
                                }
                            }

                            _ => Response::Error(
                                "Command not recognized or implemented yet.".to_string(),
                            ),
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
