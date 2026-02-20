use std::path::Path;
use tokio::fs;

const PREDATOR_SENSE_DIR: &str =
    "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense";

pub struct HardwareInterface;

impl HardwareInterface {
    async fn read_sysfs(filename: &str) -> Result<String, String> {
        let path = format!("{}/{}", PREDATOR_SENSE_DIR, filename);
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(content.trim().to_string()),
            Err(e) => Err(format!("Failed to read {}: {}", filename, e)),
        }
    }

    async fn write_sysfs(filename: &str, value: &str) -> Result<(), String> {
        let path = format!("{}/{}", PREDATOR_SENSE_DIR, filename);
        match fs::write(&path, value).await {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to write to {}: {}", filename, e)),
        }
    }

    // ==========================================
    // FAN CONTROLS
    // ==========================================

    pub async fn get_fan_mode() -> Result<String, String> {
        Self::read_sysfs("fan_speed").await
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
        let val = if enable { "1" } else { "0" };
        Self::write_sysfs("battery_limiter", val).await
    }
}
