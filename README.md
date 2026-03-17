<h1 align="center">◆ A R C H - S E N S E ◆</h1>
<h3 align="center">Acer Predator Control Center — Terminal UI for Arch Linux</h3>

<p align="center">
  A lightweight, zero-overhead Rust TUI for managing Acer Predator laptop hardware —<br>
  thermal profiles, fan control, battery management, and per-key RGB keyboard lighting —<br>
  all from a single terminal interface. No PredatorSense. No Windows. No bloat.
</p>

<p align="center">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green?style=flat-square">
  <img alt="Rust" src="https://img.shields.io/badge/rust-2024_edition-orange?style=flat-square&logo=rust">
  <img alt="Arch Linux" src="https://img.shields.io/badge/arch-linux-1793d1?style=flat-square&logo=archlinux&logoColor=white">
  <img alt="Acer Predator" src="https://img.shields.io/badge/acer-predator_PH16--71-39ff14?style=flat-square">
</p>

---

## Overview

**Arch-Sense** is a terminal-based control center that replaces Acer's Windows-only PredatorSense utility on Arch Linux. It communicates directly with hardware through:

- **sysfs** — via the [`linuwu_sense`](https://github.com/0x7375646F/Linuwu-Sense) kernel module for thermal profiles, fan speeds, battery management, and hardware toggles.
- **USB HID** — via `libusb` for keyboard RGB lighting control on the Acer Predator PH16-71, ported from the [`ph16-71-rgb`](https://github.com/Order52/ph16-71-rgb) Python project to native Rust.

Built with [`ratatui`](https://github.com/ratatui/ratatui) for a responsive, Predator-green themed interface that runs at near-zero CPU overhead.

---

## Features

### System Controls (`F1`)

| Control | Description |
|---|---|
| **Thermal Profile** | Switch between `Quiet`, `Balanced`, `Performance`, and `Low-Power` modes |
| **Fan Speed** | Manual override — Auto / Low (30%) / Medium (50%) / High (70%) / Max (100%) for CPU & GPU |
| **Battery Limiter** | Cap charging at 80% for battery longevity |
| **Battery Calibration** | Trigger battery calibration cycle (keep AC connected) |
| **Backlight Timeout** | Auto-disable keyboard RGB after 30s idle |
| **Boot Animation** | Toggle Predator boot animation & sound |
| **LCD Override** | Reduce display latency and minimize ghosting |
| **USB Charging** | Power USB ports while laptop is off (configurable threshold: 10%/20%/30%) |

### Live Sensor Monitoring

- **CPU Temperature** — read from `/sys/class/thermal/thermal_zone0/temp`
- **GPU Temperature** — queried via `nvidia-smi`
- **CPU & GPU Fan Speeds** — read from the `linuwu_sense` kernel module
- Color-coded bars: green (cool) → yellow (warm) → red (hot)

### Keyboard RGB Control (`F2`)

Full per-keyboard RGB configuration through USB HID protocol (VID: `04F2`, PID: `0117`):

| Parameter | Options |
|---|---|
| **Effects** | Off, Static, Breathing, Wave, Snake, Ripple, Neon, Rain, Lightning, Spot, Stars, Fireball, Snow, Heartbeat |
| **Colors** | Red, Orange, Gold, Green, Cyan, Blue, Purple, Magenta, Pink, White, Random |
| **Brightness** | 0–100% (mapped to hardware range 0–50) |
| **Speed** | 0–100% (mapped to hardware range 1–9, inverted) |
| **Direction** | Right, Left, Up, Down, Clockwise, Counter-CW (Wave effect only) |

### Config Persistence

RGB settings are automatically saved to `/var/lib/arch-sense/config.json` on successful apply and are restored on startup. This single system-wide location is used by both interactive `sudo arch-sense` sessions and the systemd boot service.

### Systemd Integration

A bundled `arch-sense.service` applies saved RGB settings at boot via the `--apply` headless flag — no TUI required.

---

## Prerequisites

### 1. Install the `linuwu_sense` Kernel Module

Arch-Sense depends on the `linuwu_sense` kernel module to expose Acer Predator hardware controls through sysfs. This must be installed first.

**Install `linux-headers` for your running kernel:**

```bash
sudo pacman -S linux-headers
```

**Build and install the kernel module:**

```bash
git clone https://github.com/0x7375646F/Linuwu-Sense.git
cd Linuwu-Sense
make install
```

> **Note:** `make install` compiles the module via DKMS and loads it. The module will persist across reboots.

**To uninstall the kernel module:**

```bash
cd Linuwu-Sense
make uninstall
```

**Verify the module is loaded:**

```bash
lsmod | grep linuwu_sense
ls /sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense/
```

### 2. Install `libusb`

Required for USB HID communication with the keyboard:

```bash
sudo pacman -S libusb
```

### 3. NVIDIA GPU (Optional)

GPU temperature monitoring requires `nvidia-smi`. If you have an NVIDIA GPU with the proprietary driver installed, this works out of the box. If not, GPU temperature will display as `N/A`.

---

## Installation

### Option 1 — Install via AUR Helper (`paru` / `yay`)

Install using your preferred AUR helper:

```bash
paru -S arch-sense
# or
yay -S arch-sense
```

The package manager handles service integration automatically, so no manual RGB boot setup is required for this method.

### Option 2 — Build from Source (Git Clone)

```bash
git clone https://github.com/cptdawn/Arch-Sense.git
cd Arch-Sense
cargo build --release
sudo install -Dm755 target/release/arch-sense /usr/local/bin/arch-sense
```

#### Option 2.1  —Enable RGB on Boot (Systemd) 

```bash
sudo cp arch-sense.service /etc/systemd/system/
sudo systemctl enable --now arch-sense
```

This runs `arch-sense --apply` at boot, which reads the saved config and applies RGB settings headlessly.

---

## Usage

### Launch the TUI

```bash
sudo arch-sense
```

> Root privileges are required for sysfs writes and USB device access.

### Apply Saved RGB Settings (Headless)

```bash
sudo arch-sense --apply
```

Reads `/var/lib/arch-sense/config.json` and applies the RGB configuration without launching the TUI. This is what the systemd service uses.

### Help

```bash
arch-sense --help
```

---

## Keybindings

### Global

| Key | Action |
|---|---|
| `F1` | Switch to System tab |
| `F2` | Switch to RGB tab |
| `Tab` | Toggle between tabs |
| `q` / `Ctrl+C` | Quit |

### System Tab

| Key | Action |
|---|---|
| `↑` / `k` | Navigate up |
| `↓` / `j` | Navigate down |
| `←` / `h` | Cycle option left |
| `→` / `l` | Cycle option right |
| `Enter` / `Space` | Confirm / Toggle |
| `r` | Refresh all settings |
| `Esc` | Cancel pending change |

### RGB Tab

| Key | Action |
|---|---|
| `↑` / `k` | Previous parameter |
| `↓` / `j` | Next parameter |
| `←` / `h` | Decrease / previous value |
| `→` / `l` | Increase / next value |
| `Enter` / `Space` | Apply to keyboard |
| `s` | Save config to disk |

---

## Architecture

```
arch-sense
├── src/lib.rs           # Library-first crate entry (public runtime API)
├── src/main.rs          # Thin binary shim (CLI args -> lib)
├── src/app.rs           # App state machine and event loop
├── src/ui.rs            # Ratatui rendering layer
├── src/rgb_settings.rs  # RGB protocol + hardware setting models
├── src/system.rs        # sysfs + sensor I/O
├── src/config.rs        # Persistent config model and storage
├── src/constants.rs     # Hardware paths/protocol constants
├── src/theme.rs         # Shared UI palette
├── Cargo.toml           # Rust 2024 edition, release-optimized (LTO + strip)
├── arch-sense.service   # systemd oneshot unit for boot RGB
└── LICENSE              # MIT
```

### Crate Dependencies

| Crate | Purpose |
|---|---|
| `ratatui` | Terminal UI framework |
| `crossterm` | Cross-platform terminal backend |
| `rusb` | Safe Rust bindings to `libusb` for USB HID communication |
| `anyhow` | Ergonomic error handling |
| `serde` + `serde_json` | Config serialization/deserialization |

### Hardware Communication

**sysfs (Kernel Module):**
```
/sys/module/linuwu_sense/drivers/platform:acer-wmi/acer-wmi/predator_sense/
├── fan_speed            # Read/write CPU,GPU fan percentages
├── backlight_timeout    # Keyboard backlight auto-off
├── battery_calibration  # Battery calibration trigger
├── battery_limiter      # 80% charge cap
├── boot_animation_sound # Boot animation toggle
├── lcd_override         # LCD latency reduction
└── usb_charging         # Powered-off USB charging threshold

/sys/firmware/acpi/platform_profile          # Thermal profile read/write
/sys/firmware/acpi/platform_profile_choices  # Available thermal profiles
```

**USB HID Protocol (Keyboard RGB):**
```
Device:    Acer Predator PH16-71 (VID: 0x04F2, PID: 0x0117)
Interface: 3
Endpoint:  0x04
Transfer:  Control (bmRequestType 0x21, bRequest 0x09 SET_REPORT)

Packet structure:
  Preamble:  B1 00 00 00 00 00 00 4E
  Color:     14 00 00 RR GG BB 00 00
  Effect:    08 02 OP SPEED BRIGHT COLOR_PRESET DIR 9B
```

---

## Troubleshooting

| Issue | Solution |
|---|---|
| `⚠ linuwu_sense module not loaded` | Install the kernel module (see [Prerequisites](#1-install-the-linuwu_sense-kernel-module)) |
| `Keyboard not found (VID:04F2 PID:0117)` | Ensure you're running with `sudo` and the keyboard is the Acer Predator PH16-71 |
| `Failed to detach kernel driver` | Another process may be holding the USB interface — close any other RGB software |
| GPU temperature shows `N/A` | Install NVIDIA proprietary drivers or `nvidia-smi` is not available |
| Settings show `N/A` | The kernel module is loaded but sysfs nodes aren't populated — check `dmesg` for errors |
| Permission denied on sysfs write | Must run with `sudo` |

---

## Acknowledgments

This project would not exist without the foundational work of these two projects:

- **[Linuwu-Sense](https://github.com/0x7375646F/Linuwu-Sense)** by [@0x7375646F](https://github.com/0x7375646F) — The kernel module that exposes Acer Predator hardware controls via sysfs. Arch-Sense depends entirely on this module for system-level hardware management. Massive thanks for reverse-engineering the Acer WMI interface and making it available to the Linux community.

- **[ph16-71-rgb](https://github.com/Order52/ph16-71-rgb)** by [@Order52](https://github.com/Order52) — The original Python implementation of the USB HID RGB protocol for the Acer Predator PH16-71 keyboard. The RGB control logic in Arch-Sense is a direct Rust port of this project's protocol reverse-engineering work. Huge thanks for documenting the packet structure and making keyboard RGB control possible on Linux.

---

## License

MIT — see [LICENSE](LICENSE) for details.
