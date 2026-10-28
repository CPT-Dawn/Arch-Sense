use crate::keyboard::Packet;

// RGB protocol limits
pub(crate) const BRIGHT_HW_MAX: u8 = 50; // 0x32
pub(crate) const SPEED_HW_FAST: u8 = 1;
pub(crate) const SPEED_HW_SLOW: u8 = 9;
pub(crate) const PREAMBLE: Packet = [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E];
