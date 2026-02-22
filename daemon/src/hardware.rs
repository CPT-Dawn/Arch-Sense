use tokio::fs;
use tokio::process::Command;

const PREDATOR_SENSE_PATHS: [&str; 2] = [
    "/sys/devices/platform/acer-wmi/predator_sense",
    "/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense",
];
const PLATFORM_PROFILE_PATH: &str = "/sys/firmware/acpi/platform_profile";
const PLATFORM_PROFILE_CHOICES_PATH: &str = "/sys/firmware/acpi/platform_profile_choices";

pub struct HardwareInterface;

impl HardwareInterface {
    async fn read_sysfs(filename: &str) -> Result<String, String> {
        let mut errors = Vec::new();

        for base in PREDATOR_SENSE_PATHS {
            let path = format!("{base}/{filename}");
            match fs::read_to_string(&path).await {
                Ok(content) => return Ok(content.trim().to_string()),
                Err(err) => errors.push(format!("{path}: {err}")),
            }
        }

        Err(format!("Failed to read {filename}. Tried: {}", errors.join(" | ")))
    }

    async fn write_sysfs(filename: &str, value: &str) -> Result<(), String> {
        let mut errors = Vec::new();

        for base in PREDATOR_SENSE_PATHS {
            let path = format!("{base}/{filename}");
            match fs::write(&path, value).await {
                Ok(_) => return Ok(()),
                Err(err) => errors.push(format!("{path}: {err}")),
            }
        }

        Err(format!(
            "Failed to write {filename} with value `{value}`. Tried: {}",
            errors.join(" | ")
        ))
    }

    async fn read_bool_sysfs(filename: &str) -> Result<bool, String> {
        let raw = Self::read_sysfs(filename).await?;
        parse_bool_01(&raw).ok_or_else(|| format!("Invalid {filename} value: {raw}"))
    }

    pub async fn get_gpu_temp() -> Result<u8, String> {
        let output = Command::new("nvidia-smi")
            .arg("--query-gpu=temperature.gpu")
            .arg("--format=csv,noheader")
            .output()
            .await
            .map_err(|e| format!("Failed to execute nvidia-smi: {e}"))?;

        if !output.status.success() {
            return Err("nvidia-smi failed. Ensure NVIDIA driver stack is active".to_string());
        }

        let temp_str = String::from_utf8_lossy(&output.stdout);
        temp_str
            .trim()
            .parse::<u8>()
            .map_err(|e| format!("Invalid GPU temperature output `{temp_str}`: {e}"))
    }

    pub async fn get_cpu_temp() -> Result<u8, String> {
        let raw = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .await
            .map_err(|e| format!("Could not read CPU temp: {e}"))?;
        let milli: u32 = raw
            .trim()
            .parse()
            .map_err(|e| format!("Invalid CPU temp value `{raw}`: {e}"))?;
        Ok((milli / 1000) as u8)
    }

    pub async fn get_fan_speed() -> Result<(u8, u8), String> {
        let raw = Self::read_sysfs("fan_speed").await?;
        let (cpu_raw, gpu_raw) = raw
            .split_once(',')
            .ok_or_else(|| format!("Invalid fan_speed format: {raw}"))?;

        let cpu = cpu_raw
            .trim()
            .parse::<u8>()
            .map_err(|e| format!("Invalid CPU fan value `{cpu_raw}`: {e}"))?;
        let gpu = gpu_raw
            .trim()
            .parse::<u8>()
            .map_err(|e| format!("Invalid GPU fan value `{gpu_raw}`: {e}"))?;

        Ok((cpu.min(100), gpu.min(100)))
    }

    pub async fn set_fan_mode(mode: shared::FanMode) -> Result<(), String> {
        let value = match mode {
            shared::FanMode::Auto => "0,0".to_string(),
            shared::FanMode::Quiet => "30,30".to_string(),
            shared::FanMode::Balanced => "50,50".to_string(),
            shared::FanMode::Performance => "70,70".to_string(),
            shared::FanMode::Turbo => "100,100".to_string(),
            shared::FanMode::Custom(cpu, gpu) => format!("{},{}", cpu.min(100), gpu.min(100)),
        };

        Self::write_sysfs("fan_speed", &value).await
    }

    pub async fn get_backlight_timeout() -> Result<bool, String> {
        Self::read_bool_sysfs("backlight_timeout").await
    }

    pub async fn set_backlight_timeout(enable: bool) -> Result<(), String> {
        Self::write_sysfs("backlight_timeout", if enable { "1\n" } else { "0\n" }).await
    }

    pub async fn get_battery_calibration() -> Result<bool, String> {
        Self::read_bool_sysfs("battery_calibration").await
    }

    pub async fn set_battery_calibration(enable: bool) -> Result<(), String> {
        Self::write_sysfs("battery_calibration", if enable { "1\n" } else { "0\n" }).await
    }

    pub async fn get_battery_limiter() -> Result<bool, String> {
        Self::read_bool_sysfs("battery_limiter").await
    }

    pub async fn set_battery_limiter(enable: bool) -> Result<(), String> {
        Self::write_sysfs("battery_limiter", if enable { "1\n" } else { "0\n" }).await
    }

    pub async fn get_boot_animation() -> Result<bool, String> {
        Self::read_bool_sysfs("boot_animation_sound").await
    }

    pub async fn set_boot_animation(enable: bool) -> Result<(), String> {
        Self::write_sysfs("boot_animation_sound", if enable { "1\n" } else { "0\n" }).await
    }

    pub async fn get_lcd_overdrive() -> Result<bool, String> {
        Self::read_bool_sysfs("lcd_override").await
    }

    pub async fn set_lcd_overdrive(enable: bool) -> Result<(), String> {
        Self::write_sysfs("lcd_override", if enable { "1\n" } else { "0\n" }).await
    }

    pub async fn get_usb_charging() -> Result<u8, String> {
        let raw = Self::read_sysfs("usb_charging").await?;
        let threshold = raw
            .parse::<u8>()
            .map_err(|e| format!("Invalid usb_charging value `{raw}`: {e}"))?;

        if [0, 10, 20, 30].contains(&threshold) {
            Ok(threshold)
        } else {
            Err(format!("Unsupported usb_charging value: {threshold}"))
        }
    }

    pub async fn set_usb_charging(threshold: u8) -> Result<(), String> {
        if ![0, 10, 20, 30].contains(&threshold) {
            return Err("USB threshold must be one of 0, 10, 20, 30".to_string());
        }

        Self::write_sysfs("usb_charging", &format!("{threshold}\n")).await
    }

    pub async fn get_thermal_profile() -> Result<String, String> {
        let profile = fs::read_to_string(PLATFORM_PROFILE_PATH)
            .await
            .map_err(|e| format!("Failed to read thermal profile: {e}"))?;
        Ok(profile.trim().to_string())
    }

    pub async fn get_thermal_profile_choices() -> Result<Vec<String>, String> {
        let raw = fs::read_to_string(PLATFORM_PROFILE_CHOICES_PATH)
            .await
            .map_err(|e| format!("Failed to read thermal profile choices: {e}"))?;

        Ok(raw
            .split_whitespace()
            .map(|entry| entry.trim().trim_start_matches('[').trim_end_matches(']').to_string())
            .filter(|entry| !entry.is_empty())
            .collect())
    }

    pub async fn set_thermal_profile(profile: &str) -> Result<(), String> {
        let profile = profile.trim();
        if profile.is_empty() {
            return Err("Thermal profile must not be empty".to_string());
        }

        let choices = Self::get_thermal_profile_choices().await?;
        if !choices.iter().any(|choice| choice == profile) {
            return Err(format!(
                "Unsupported thermal profile `{profile}`. Supported: {}",
                choices.join(", ")
            ));
        }

        fs::write(PLATFORM_PROFILE_PATH, format!("{profile}\n"))
            .await
            .map_err(|e| format!("Failed to set thermal profile to `{profile}`: {e}"))
    }
}

fn parse_bool_01(raw: &str) -> Option<bool> {
    match raw.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}
