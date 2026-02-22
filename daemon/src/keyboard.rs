use rusb::{Direction, Recipient, RequestType, request_type};
use std::time::Duration;

// Your specific Acer Predator PH16-71 Hardware IDs
const VID: u16 = 0x04F2;
const PID: u16 = 0x0117;
const INTERFACE: u8 = 3;
const ENDPOINT: u8 = 0x04; // The USB OUT endpoint for lighting

pub struct KeyboardInterface;

impl KeyboardInterface {
    pub fn supported_effects() -> &'static [&'static str] {
        &[
            "neon",
            "wave",
            "breath",
            "rainbow",
            "reactive",
            "ripple",
            "starlight",
            "rain",
            "fire",
            "aurora",
        ]
    }

    pub fn set_global_color(r: u8, g: u8, b: u8, brightness: u8) -> Result<(), String> {
        let mut handle = rusb::open_device_with_vid_pid(VID, PID)
            .ok_or("❌ Could not find Acer USB Keyboard! Is the Daemon running as root?")?;

        let _ = handle.set_auto_detach_kernel_driver(true);
        handle
            .claim_interface(INTERFACE)
            .map_err(|e| format!("USB claim failed: {}", e))?;

        let req_type = request_type(Direction::Out, RequestType::Class, Recipient::Interface);
        let timeout = Duration::from_millis(500);

        let init_payload = [0x12, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0xe5];
        Self::write_control_checked(&mut handle, req_type, &init_payload, timeout)?;

        let mut color_data = vec![0u8; 1024];
        let level = brightness.min(100) as u16;
        let scaled_r = ((r as u16 * level) / 100) as u8;
        let scaled_g = ((g as u16 * level) / 100) as u8;
        let scaled_b = ((b as u16 * level) / 100) as u8;

        for chunk in color_data.chunks_mut(4) {
            chunk[0] = 0x00;
            chunk[1] = scaled_r;
            chunk[2] = scaled_g;
            chunk[3] = scaled_b;
        }

        for chunk in color_data.chunks(64) {
            Self::write_interrupt_checked(&mut handle, chunk, timeout)?;
        }

        let apply_payload = [0x08, 0x02, 0x33, 0x05, 0x32, 0x08, 0x01, 0x82];
        Self::write_control_checked(&mut handle, req_type, &apply_payload, timeout)?;

        let _ = handle.release_interface(INTERFACE);
        Ok(())
    }

    pub fn set_animation(effect: &str, speed: u8, brightness: u8) -> Result<(), String> {
        let effect = effect.to_ascii_lowercase();
        let speed = speed.clamp(1, 10);
        let brightness = brightness.min(100);

        let mut handle = rusb::open_device_with_vid_pid(VID, PID)
            .ok_or("❌ Could not find Acer USB Keyboard!")?;

        let _ = handle.set_auto_detach_kernel_driver(true);
        handle
            .claim_interface(INTERFACE)
            .map_err(|e| format!("USB claim failed: {}", e))?;

        let req_type = request_type(Direction::Out, RequestType::Class, Recipient::Interface);
        let timeout = Duration::from_millis(500);

        let init_payload = [0xb1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4e];
        Self::write_control_checked(&mut handle, req_type, &init_payload, timeout)?;

        let apply_payload = Self::effect_payload(&effect, speed, brightness)?;
        Self::write_control_checked(&mut handle, req_type, &apply_payload, timeout)?;

        let _ = handle.release_interface(INTERFACE);
        Ok(())
    }

    fn effect_payload(effect: &str, speed: u8, brightness: u8) -> Result<[u8; 8], String> {
        let (effect_code, direction) = match effect {
            "neon" => (0x08, 0x01),
            "wave" => (0x03, 0x02),
            "breath" => (0x02, 0x01),
            "rainbow" => (0x04, 0x01),
            "reactive" => (0x05, 0x01),
            "ripple" => (0x06, 0x01),
            "starlight" => (0x07, 0x01),
            "rain" => (0x09, 0x01),
            "fire" => (0x0A, 0x01),
            "aurora" => (0x0B, 0x01),
            _ => {
                return Err(format!(
                    "Unknown animation '{}'. Supported: {}",
                    effect,
                    Self::supported_effects().join(", ")
                ));
            }
        };

        Ok([0x08, 0x02, effect_code, 0x01, brightness, speed, direction, 0x9b])
    }

    fn write_control_checked(
        handle: &mut rusb::DeviceHandle<rusb::GlobalContext>,
        req_type: u8,
        payload: &[u8],
        timeout: Duration,
    ) -> Result<(), String> {
        handle
            .write_control(req_type, 9, 0x0300, INTERFACE as u16, payload, timeout)
            .map(|_| ())
            .map_err(|e| format!("USB control write failed: {}", e))
    }

    fn write_interrupt_checked(
        handle: &mut rusb::DeviceHandle<rusb::GlobalContext>,
        payload: &[u8],
        timeout: Duration,
    ) -> Result<(), String> {
        handle
            .write_interrupt(ENDPOINT, payload, timeout)
            .map(|_| ())
            .map_err(|e| format!("USB interrupt write failed: {}", e))
    }
}
