use tokio::fs;
use tokio::process::Command;

// The base path created by the linuwu_sense kernel module
const PREDATOR_SENSE_DIR: &str = "/sys/devices/platform/acer-wmi/predator_sense";

pub struct HardwareInterface;

impl HardwareInterface {
    /// Helper to cleanly read from sysfs
    async fn read_sysfs(filename: &str) -> Result<String, String> {
        let path = format!("{}/{}", PREDATOR_SENSE_DIR, filename);
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(content.trim().to_string()),
            Err(e) => Err(format!("❌ Failed to read {}: {}", filename, e)),
        }
    }

    /// Helper to cleanly write to sysfs
    async fn write_sysfs(filename: &str, value: &str) -> Result<(), String> {
        let path = format!("{}/{}", PREDATOR_SENSE_DIR, filename);
        match fs::write(&path, value).await {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("❌ Failed to write to {}: {}", filename, e)),
        }
    }

    /// Executes nvidia-smi to get the current discrete GPU temperature
    pub async fn get_gpu_temp() -> Result<u8, String> {
        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=temperature.gpu")
            .arg("--format=csv,noheader")
            .output()
            .await
            .map_err(|e| format!("❌ Failed to execute nvidia-smi: {}", e))?;

        if output.status.success() {
            let temp_str = String::from_utf8_lossy(&output.stdout);
            // nvidia-smi outputs a string like "45\n", so we trim and parse it
            let temp: u8 = temp_str.trim().parse().unwrap_or(0);
            Ok(temp)
        } else {
            Err("❌ nvidia-smi failed. Is the NVIDIA driver installed and active?".to_string())
        }
    }

    // ==========================================
    // FAN CONTROLS
    // ==========================================

    /// Gets the current fan speed as percentages (CPU, GPU)
    pub async fn get_fan_speed() -> Result<(u8, u8), String> {
        let data = Self::read_sysfs("fan_speed").await?;
        let parts: Vec<&str> = data.split(',').collect();
        if parts.len() == 2 {
            let cpu = parts[0].parse().unwrap_or(0);
            let gpu = parts[1].parse().unwrap_or(0);
            Ok((cpu, gpu))
        } else {
            Err("❌ Invalid data format".to_string())
        }
    }

    /// Reads the actual hardware temperature from the Linux thermal zone
    pub async fn get_cpu_temp() -> Result<u8, String> {
        // Read the raw thermal file
        match fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").await {
            Ok(raw) => {
                let temp_millidegrees: f32 = raw.trim().parse().unwrap_or(0.0);
                // Convert to Celsius
                Ok((temp_millidegrees / 1000.0) as u8)
            }
            Err(e) => Err(format!("❌ Could not read temp: {}", e)),
        }
    }

    pub async fn set_fan_mode(mode: shared::FanMode) -> Result<(), String> {
        let val = match mode {
            shared::FanMode::Auto => "0,0".to_string(),
            shared::FanMode::Quiet => "30,30".to_string(),
            shared::FanMode::Balanced => "50,50".to_string(),
            shared::FanMode::Performance => "70,70".to_string(),
            shared::FanMode::Turbo => "100,100".to_string(),
            shared::FanMode::Custom(cpu, gpu) => format!("{},{}", cpu, gpu),
        };

        Self::write_sysfs("fan_speed", &val).await
    }

    // ==========================================
    // 🔋 BATTERY
    // ==========================================

    pub async fn set_battery_limiter(enable: bool) -> Result<(), String> {
        let val = if enable { "1\n" } else { "0\n" };
        Self::write_sysfs("battery_limiter", val).await
    }

    pub async fn set_battery_calibration(enable: bool) -> Result<(), String> {
        let val = if enable { "1\n" } else { "0\n" };
        Self::write_sysfs("battery_calibration", val).await
    }

    // ==========================================
    // ⚙️ SYSTEM & DISPLAY
    // ==========================================

    pub async fn set_lcd_overdrive(enable: bool) -> Result<(), String> {
        let val = if enable { "1\n" } else { "0\n" };
        Self::write_sysfs("lcd_override", val).await
    }

    pub async fn set_boot_animation(enable: bool) -> Result<(), String> {
        let val = if enable { "1\n" } else { "0\n" };
        Self::write_sysfs("boot_animation_sound", val).await
    }

    // ==========================================
    // 💡 KEYBOARD/USB EXTRAS
    // ==========================================

    pub async fn set_backlight_timeout(enable: bool) -> Result<(), String> {
        let val = if enable { "1\n" } else { "0\n" };
        Self::write_sysfs("backlight_timeout", val).await
    }

    pub async fn set_usb_charging(threshold: u8) -> Result<(), String> {
        if ![0, 10, 20, 30].contains(&threshold) {
            return Err("❌ USB threshold must be 0, 10, 20, or 30".to_string());
        }
        Self::write_sysfs("usb_charging", &format!("{}\n", threshold)).await
    }
}
