mod hardware;
use hardware::HardwareInterface;

#[tokio::main]
async fn main() {
    println!("🔥 Starting Arch-Sense Daemon...");

    match HardwareInterface::get_fan_mode().await {
        Ok(mode) => println!("📊 Current Fan Hardware State: {}", mode),
        Err(e) => eprintln!("❌ Error: {}", e),
    }

    match HardwareInterface::set_battery_limiter(true).await {
        Ok(_) => println!("✅ Battery safely limited to 80% capacity!"),
        Err(e) => eprintln!("❌ Error setting battery limit: {}", e),
    }
}
