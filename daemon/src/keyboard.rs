use rusb::{Direction, Recipient, RequestType, request_type};
use shared::RgbMode;
use std::time::Duration;

// Your specific Acer Predator PH16-71 Hardware IDs
const VID: u16 = 0x04F2;
const PID: u16 = 0x0117;
const INTERFACE: u8 = 3;
const ENDPOINT: u8 = 0x04; // The USB OUT endpoint for lighting

pub struct KeyboardInterface;

impl KeyboardInterface {
    pub fn apply_mode(mode: &RgbMode, brightness: u8, _fx_speed: u8) -> Result<(), String> {
        let brightness = brightness.clamp(0, 100);
        match mode {
            RgbMode::Solid(color) => {
                let (r, g, b) = color.rgb();
                Self::set_global_color(
                    scale_by_brightness(r, brightness),
                    scale_by_brightness(g, brightness),
                    scale_by_brightness(b, brightness),
                )
            }
            RgbMode::Wave => Self::set_animation(Animation::Wave),
            RgbMode::Neon => Self::set_animation(Animation::Neon),
        }
    }

    fn set_global_color(r: u8, g: u8, b: u8) -> Result<(), String> {
        Self::with_claimed_handle(|handle| {
            let req_type = request_type(Direction::Out, RequestType::Class, Recipient::Interface);
            let timeout = Duration::from_millis(500);

            let init_payload = [0x12, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0xe5];
            handle
                .write_control(
                    req_type,
                    9,
                    0x0300,
                    INTERFACE as u16,
                    &init_payload,
                    timeout,
                )
                .map_err(|e| format!("USB init handshake failed: {}", e))?;

            let mut color_data = vec![0u8; 1024];
            for chunk in color_data.chunks_mut(4) {
                chunk[0] = 0x00;
                chunk[1] = r;
                chunk[2] = g;
                chunk[3] = b;
            }

            for chunk in color_data.chunks(64) {
                handle
                    .write_interrupt(ENDPOINT, chunk, timeout)
                    .map_err(|e| format!("USB color payload transfer failed: {}", e))?;
            }

            let apply_payload = [0x08, 0x02, 0x33, 0x05, 0x32, 0x08, 0x01, 0x82];
            handle
                .write_control(
                    req_type,
                    9,
                    0x0300,
                    INTERFACE as u16,
                    &apply_payload,
                    timeout,
                )
                .map_err(|e| format!("USB apply command failed: {}", e))?;

            Ok(())
        })
    }

    fn set_animation(animation: Animation) -> Result<(), String> {
        Self::with_claimed_handle(|handle| {
            let req_type = request_type(Direction::Out, RequestType::Class, Recipient::Interface);
            let timeout = Duration::from_millis(500);

            let init_payload = [0xb1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4e];
            handle
                .write_control(
                    req_type,
                    9,
                    0x0300,
                    INTERFACE as u16,
                    &init_payload,
                    timeout,
                )
                .map_err(|e| format!("USB animation init failed: {}", e))?;

            let apply_payload = match animation {
                Animation::Neon => [0x08, 0x02, 0x08, 0x01, 0x32, 0x01, 0x01, 0x9b],
                Animation::Wave => [0x08, 0x02, 0x03, 0x01, 0x32, 0x01, 0x02, 0x9b],
            };

            handle
                .write_control(
                    req_type,
                    9,
                    0x0300,
                    INTERFACE as u16,
                    &apply_payload,
                    timeout,
                )
                .map_err(|e| format!("USB animation apply failed: {}", e))?;

            Ok(())
        })
    }

    fn with_claimed_handle<F>(operation: F) -> Result<(), String>
    where
        F: FnOnce(&mut rusb::DeviceHandle<rusb::GlobalContext>) -> Result<(), String>,
    {
        let mut handle = rusb::open_device_with_vid_pid(VID, PID)
            .ok_or("Could not find Acer USB Keyboard device")?;

        let _ = handle.set_auto_detach_kernel_driver(true);
        handle
            .claim_interface(INTERFACE)
            .map_err(|e| format!("USB claim failed: {}", e))?;

        let result = operation(&mut handle);
        let release_result = handle.release_interface(INTERFACE);

        match (result, release_result) {
            (Err(e), _) => Err(e),
            (Ok(_), Err(e)) => Err(format!("USB release interface failed: {}", e)),
            (Ok(_), Ok(_)) => Ok(()),
        }
    }
}

#[derive(Clone, Copy)]
enum Animation {
    Wave,
    Neon,
}

fn scale_by_brightness(channel: u8, brightness: u8) -> u8 {
    (((channel as u16) * (brightness as u16)) / 100) as u8
}
