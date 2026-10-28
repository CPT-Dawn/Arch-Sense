use crate::keyboard::Packet;

// RGB protocol limits
pub(crate) const BRIGHT_HW_MAX: u8 = 50; // 0x32
pub(crate) const SPEED_HW_FAST: u8 = 1;
pub(crate) const SPEED_HW_SLOW: u8 = 9;
pub(crate) const PREAMBLE: Packet = [0xB1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4E];

/// What to tell a user who hits a permission error. The package grants the
/// `arch-sense` group access to the hardware; membership is the one manual step.
pub(crate) fn setup_hint() -> &'static str {
    "join the `arch-sense` group with `sudo usermod -aG arch-sense $USER`, then log out and back in"
}
