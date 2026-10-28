//! The USB HID keyboard: finding it, and the transport that talks to it.
//!
//! All USB HID I/O goes through [`KeyboardTransport`], which has a single
//! "send these packets" operation. [`RusbTransport`] talks to the real Chicony
//! `04f2:0117` controller; [`RecordingTransport`] remembers what it was sent so
//! tests and demo mode can run without the hardware.
//!
//! Finding the keyboard and checking access is a read of the machine root
//! (sysfs plus the usbfs device node), so it is fakeable too.

use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use rusb::{DeviceHandle, Error as UsbError, GlobalContext};

use crate::constants::setup_hint;
use crate::machine::MachineRoot;

/// One HID feature report the keyboard controller understands.
pub type Packet = [u8; 8];

/// USB vendor and product ID of the PH16-71-family keyboard controller.
pub const USB_HID_VENDOR_ID: u16 = 0x04F2;
pub const USB_HID_PRODUCT_ID: u16 = 0x0117;
const INTERFACE: u8 = 3;
const ENDPOINT: u8 = 0x04;
const TIMEOUT: Duration = Duration::from_millis(1000);

/// Sends lighting packets to the keyboard.
pub trait KeyboardTransport: Send {
    /// Sends `packets` in order, as one transaction.
    fn send(&mut self, packets: &[Packet]) -> Result<()>;
}

/// Remembers every transaction instead of touching USB. Clones share one log.
#[derive(Clone, Debug, Default)]
pub struct RecordingTransport {
    sent: Arc<Mutex<Vec<Vec<Packet>>>>,
}

impl RecordingTransport {
    /// Every transaction sent so far, oldest first.
    pub fn sent(&self) -> Vec<Vec<Packet>> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl KeyboardTransport for RecordingTransport {
    fn send(&mut self, packets: &[Packet]) -> Result<()> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(packets.to_vec());
        Ok(())
    }
}

/// The real keyboard, over libusb.
#[derive(Clone, Copy, Debug, Default)]
pub struct RusbTransport;

impl KeyboardTransport for RusbTransport {
    fn send(&mut self, packets: &[Packet]) -> Result<()> {
        let handle = open_keyboard()?;
        let was_attached = handle.kernel_driver_active(INTERFACE).unwrap_or(false);

        if was_attached {
            handle.detach_kernel_driver(INTERFACE).with_context(|| {
                format!(
                    "failed to detach keyboard kernel driver on interface {INTERFACE}; {}",
                    setup_hint()
                )
            })?;
        }

        if let Err(error) = handle.claim_interface(INTERFACE).with_context(|| {
            format!(
                "failed to claim USB interface {INTERFACE}; {}",
                setup_hint()
            )
        }) {
            if was_attached {
                let _ = handle.attach_kernel_driver(INTERFACE);
            }
            return Err(error);
        }

        let _ = handle.clear_halt(ENDPOINT);

        let transfer = (|| -> Result<()> {
            for packet in packets {
                handle
                    .write_control(0x21, 0x09, 0x0300, INTERFACE as u16, packet, TIMEOUT)
                    .with_context(|| {
                        format!("USB control transfer failed for packet {packet:02X?}")
                    })?;
            }
            Ok(())
        })();

        let release = handle
            .release_interface(INTERFACE)
            .context("failed to release USB keyboard interface");

        if was_attached {
            let _ = handle.attach_kernel_driver(INTERFACE);
        }

        transfer?;
        release?;
        Ok(())
    }
}

fn open_keyboard() -> Result<DeviceHandle<GlobalContext>> {
    let devices = rusb::devices().context("listing USB devices")?;
    let mut found = false;
    let mut access_denied = false;
    let mut last_error = None;

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(desc) => desc,
            Err(err) => {
                last_error = Some(err.to_string());
                continue;
            }
        };

        if desc.vendor_id() != USB_HID_VENDOR_ID || desc.product_id() != USB_HID_PRODUCT_ID {
            continue;
        }

        found = true;
        match device.open() {
            Ok(handle) => return Ok(handle),
            Err(UsbError::Access) => access_denied = true,
            Err(err) => last_error = Some(err.to_string()),
        }
    }

    if access_denied {
        bail!(
            "Keyboard USB access denied (VID:04F2 PID:0117); {}",
            setup_hint()
        )
    } else if found {
        bail!(
            "Keyboard found but could not be opened: {}",
            last_error.unwrap_or_else(|| "unknown USB error".to_string())
        )
    } else {
        bail!("Keyboard not found (VID:04F2 PID:0117)")
    }
}

/// Whether the USB HID keyboard is plugged in and openable by this user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsbAccess {
    Accessible,
    PermissionDenied,
    NotFound,
    Error(String),
}

/// Looks for the USB HID keyboard under the machine root and checks we may open it.
pub fn keyboard_access(machine: &MachineRoot) -> UsbAccess {
    let devices_dir = machine.usb_devices_dir();
    let entries = match fs::read_dir(&devices_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return UsbAccess::NotFound,
        Err(error) => {
            return UsbAccess::Error(format!("reading {}: {error}", devices_dir.display()));
        }
    };

    let mut result = UsbAccess::NotFound;
    for entry in entries.flatten() {
        let device = entry.path();
        if !is_usb_hid_keyboard(&device) {
            continue;
        }

        let (Some(bus), Some(number)) = (read_u16(&device, "busnum"), read_u16(&device, "devnum"))
        else {
            continue;
        };

        let node = machine.usb_device_node(bus, number);
        result = match OpenOptions::new().read(true).write(true).open(&node) {
            Ok(_) => return UsbAccess::Accessible,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                UsbAccess::PermissionDenied
            }
            Err(error) => UsbAccess::Error(format!(
                "Keyboard found but could not be opened: {}: {error}",
                node.display()
            )),
        };
    }

    result
}

fn is_usb_hid_keyboard(device: &Path) -> bool {
    read_hex(device, "idVendor") == Some(USB_HID_VENDOR_ID)
        && read_hex(device, "idProduct") == Some(USB_HID_PRODUCT_ID)
}

fn read_hex(device: &Path, attr: &str) -> Option<u16> {
    let raw = fs::read_to_string(device.join(attr)).ok()?;
    u16::from_str_radix(raw.trim(), 16).ok()
}

fn read_u16(device: &Path, attr: &str) -> Option<u16> {
    fs::read_to_string(device.join(attr))
        .ok()?
        .trim()
        .parse()
        .ok()
}
