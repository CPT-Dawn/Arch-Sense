use rusb::{Direction, Recipient, RequestType, request_type};
use std::time::Duration;

// Your specific Acer Predator PH16-71 Hardware IDs
const VID: u16 = 0x04F2;
const PID: u16 = 0x0117;
const INTERFACE: u8 = 3;
const ENDPOINT: u8 = 0x04; // The USB OUT endpoint for lighting

pub struct KeyboardInterface;

impl KeyboardInterface {
    pub fn set_global_color(r: u8, g: u8, b: u8) -> Result<(), String> {
        // 1. Find the Keyboard on the USB Bus
        let handle = rusb::open_device_with_vid_pid(VID, PID)
            .ok_or("❌ Could not find Acer USB Keyboard! Is the Daemon running as root?")?;

        // 2. Hijack the device from the Linux Kernel temporarily
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle
            .claim_interface(INTERFACE)
            .map_err(|e| format!("USB Claim failed: {}", e))?;

        let req_type = request_type(Direction::Out, RequestType::Class, Recipient::Interface);
        let timeout = Duration::from_millis(500);

        // 3. Send the Initialization Handshake (From Python reverse-engineering)
        let init_payload = [0x12, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0xe5];
        let _ = handle.write_control(
            req_type,
            9,
            0x0300,
            INTERFACE as u16,
            &init_payload,
            timeout,
        );

        // 4. Build the massive 1024-byte color map (4 bytes per key: [0x00, R, G, B])
        let mut color_data = vec![0u8; 1024];
        for chunk in color_data.chunks_mut(4) {
            chunk[0] = 0x00;
            chunk[1] = r;
            chunk[2] = g;
            chunk[3] = b;
        }

        // 5. Blast the data to Endpoint 4 in chunks of 64 bytes
        for chunk in color_data.chunks(64) {
            let _ = handle.write_interrupt(ENDPOINT, chunk, timeout);
        }

        // 6. Send the Apply/Commit Command
        let apply_payload = [0x08, 0x02, 0x33, 0x05, 0x32, 0x08, 0x01, 0x82];
        let _ = handle.write_control(
            req_type,
            9,
            0x0300,
            INTERFACE as u16,
            &apply_payload,
            timeout,
        );

        // 7. Give the keyboard back to Linux!
        let _ = handle.release_interface(INTERFACE);
        Ok(())
    }

    pub fn set_animation(effect: &str) -> Result<(), String> {
        let mut handle = rusb::open_device_with_vid_pid(VID, PID)
            .ok_or("❌ Could not find Acer USB Keyboard!")?;

        let _ = handle.set_auto_detach_kernel_driver(true);
        handle
            .claim_interface(INTERFACE)
            .map_err(|e| format!("USB Claim failed: {}", e))?;

        let req_type = request_type(Direction::Out, RequestType::Class, Recipient::Interface);
        let timeout = Duration::from_millis(500);

        // 1. Send the Hardware Effect Init sequence (Different from static color!)
        let init_payload = [0xb1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4e];
        let _ = handle.write_control(
            req_type,
            9,
            0x0300,
            INTERFACE as u16,
            &init_payload,
            timeout,
        );

        // 2. Match the effect to the reverse-engineered Acer payloads
        let apply_payload = match effect {
            "neon" => [0x08, 0x02, 0x08, 0x01, 0x32, 0x01, 0x01, 0x9b], // Smooth RGB shifting
            "wave" => [0x08, 0x02, 0x03, 0x01, 0x32, 0x01, 0x02, 0x9b], // Rainbow wave (Left direction)
            "breath" => [0x08, 0x02, 0x02, 0x01, 0x32, 0x07, 0x01, 0x9b], // White breathing
            _ => return Err("Unknown animation effect".to_string()),
        };

        // 3. Blast the effect command
        let _ = handle.write_control(
            req_type,
            9,
            0x0300,
            INTERFACE as u16,
            &apply_payload,
            timeout,
        );

        let _ = handle.release_interface(INTERFACE);
        Ok(())
    }
}
