<h1 align="center">◆ A R C H - S E N S E ◆</h1>
<h3 align="center">Acer Predator Control Center — Terminal UI for Arch Linux</h3>

<p align="center">
  A lightweight, zero-overhead Rust TUI for managing Acer Predator laptop hardware —<br>
  thermal profiles, fan control, battery management, and per-key RGB keyboard lighting —<br>
  all from a single terminal interface. No PredatorSense. No Windows. No bloat.
</p>

<p align="center">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green?style=flat-square">
  <img alt="Rust" src="https://img.shields.io/badge/rust-2021_edition-orange?style=flat-square&logo=rust">
  <img alt="Arch Linux" src="https://img.shields.io/badge/arch-linux-1793d1?style=flat-square&logo=archlinux&logoColor=white">
  <img alt="Acer Predator" src="https://img.shields.io/badge/acer-predator_PH16--71-39ff14?style=flat-square">
</p>

---

## Overview

**Arch-Sense** is a terminal-based control center that replaces Acer's Windows-only PredatorSense utility on Arch Linux. It communicates directly with the hardware through:

- **sysfs** — via the [`linuwu_sense`](https://github.com/0x7375646F/Linuwu-Sense) kernel module for thermal profiles, fan speeds, battery management, and hardware toggles.
- **USB HID** — via `libusb` for keyboard RGB lighting control on the Acer Predator PH16-71, ported from the [`ph16-71-rgb`](https://github.com/Order52/ph16-71-rgb) Python project to native Rust.

Built with [`ratatui`](https://github.com/ratatui/ratatui) as a modern single-screen TUI with a dark professional palette, animated gauges, keyboard-first navigation, and a non-blocking hardware worker thread.

---

## Features

### ⚙ Controls Panel

| Control | Description |
|---|---|
| **Thermal Profile** | Switch between `Quiet`, `Balanced`, `Performance`, and `Low-Power` modes. |
| **Fan Speed** | Manual override — Auto / Low (30%) / Medium (50%) / High (70%) / Max (100%) for CPU & GPU. |
| **Battery Limiter** | Cap charging at 80% for battery longevity. |
| **Battery Calibration** | Trigger a battery calibration cycle (keep AC connected). |
| **Backlight Timeout** | Auto-disable keyboard RGB after 30s of idle time. |
| **Boot Animation** | Toggle the Acer Predator boot animation & sound. |
| **LCD Override** | Reduce display latency and minimize ghosting. |
| **USB Charging** | Power USB ports while the laptop is off (configurable threshold: 10% / 20% / 30%). |

### ⌨ Keyboard RGB Panel

Full per-keyboard RGB configuration through USB HID protocol (VID: `04F2`, PID: `0117`):

| Parameter | Options |
|---|---|
| **Mode** | Off, Static, Breathing, Wave, Snake, Ripple, Rainbow, Rain, Lightning, Spot, Stars, Fireball, Snow, Heartbeat |
| **Color** | Red, Orange, Gold, Emerald, Cyan, Blue, Violet, Magenta, Pink, White, Random |
| **Brightness** | 0–100% |
| **Speed** | 0–100% |
| **Direction** | Right, Left, Up, Down, Clockwise, Counter-CW (Wave effect only) |

*RGB settings are automatically saved to `/var/lib/arch-sense/config.json` on successful apply and are restored on startup.*

### 📊 Live Sensor Monitoring

- **CPU Temperature** — read directly from `/sys/class/thermal/thermal_zone0/temp`.
- **GPU Temperature** — queried via `nvidia-smi` (requires proprietary NVIDIA drivers).
- **CPU & GPU Fan Speeds** — read from the `linuwu_sense` kernel module.
- Features animated charts with cool, warning, and hot status colors.

---

## Prerequisites

### 1. Install the `linuwu_sense` Kernel Module

Arch-Sense **strictly depends** on the `linuwu_sense` kernel module to expose Acer Predator hardware controls through sysfs.

**Install `linux-headers` for your running kernel:**

```bash
sudo pacman -S linux-headers
```

**Build and install the kernel module via DKMS:**

```bash
git clone https://github.com/0x7375646F/Linuwu-Sense.git
cd Linuwu-Sense
make install
```

> **Note:** `make install` compiles the module and loads it. The module will persist across reboots via DKMS. Ensure you see the `linuwu_sense` directory under `/sys/module/` after installation.

### 2. Install `libusb`

Required for USB HID communication with the keyboard:

```bash
sudo pacman -S libusb
```

---

## Installation (AUR)

The recommended way to install Arch-Sense is via the Arch User Repository (AUR).

```bash
paru -S arch-sense
# or
yay -S arch-sense
```

### Post-Installation Setup (Crucial)

Arch-Sense is designed to run as a **normal user** without `sudo`. To enable this, you must run the permission setup command once after installation:

```bash
arch-sense --install-permissions
```

This command uses `pkexec` (Polkit) to:
1. Create an `arch-sense` system group.
2. Install a `udev` rule for the Acer keyboard USB device (`04f2:0117`).
3. Set up a systemd service (`arch-sense-permissions.service`) to re-apply sysfs permissions automatically on boot.
4. Add your current user to the `arch-sense` group.

**Important:** After running the command, you **must log out and log back in** (or reboot) for the group changes to take effect.

### Headless Boot Persistence (Systemd)

When installed via the AUR, the `arch-sense.service` is automatically installed. This service runs `arch-sense --apply` headlessly on boot, reading `/var/lib/arch-sense/config.json` and applying your last saved RGB configuration before you even reach the login screen.

If you ever need to manually enable it:
```bash
sudo systemctl enable --now arch-sense.service
```

---

## Usage

### Launch the TUI

Simply open your terminal and run:

```bash
arch-sense
```

### Navigation

The TUI utilizes a fully keyboard-driven, context-sensitive footer.

- `⇥ Tab` / `Shift+Tab` — Switch focus between the panels (Controls, Keyboard, Sensors).
- `↑↓` — Navigate lists or select fields.
- `←→` — Adjust values or choose options.
- `↵ Enter` — Apply changes or toggle states.
- `R` — Refresh sensor data (when focused on Sensors).
- `Q` — Quit the application.

### Diagnostics & Troubleshooting

To check hardware permissions and system status without launching the UI:

```bash
arch-sense --doctor
```

---

## Expected Errors & Solutions

Arch-Sense features a robust status footer (●) that will alert you to hardware and permission issues.

| Error Message | Cause & Solution |
|---|---|
| `● Kernel Module Missing` | The `linuwu_sense` module is not loaded into the kernel. Ensure you have installed it following the [Prerequisites](#1-install-the-linuwu_sense-kernel-module) section. If you recently updated your kernel, you may need to ensure your DKMS modules rebuilt successfully. |
| `● USB Permission Denied` | Your user does not have permission to access the raw USB device. Ensure you have run `arch-sense --install-permissions` and **logged out and back in** to apply the new `arch-sense` group. |
| `● Keyboard Not Found` | Arch-Sense could not find a USB device matching the Acer Predator PH16-71 Vendor/Product IDs (`VID:04F2 PID:0117`). Ensure your specific laptop model is supported. |
| GPU Temp shows `N/A` | `nvidia-smi` is not installed or the proprietary NVIDIA drivers are not active. If using an integrated GPU, this is expected behavior. |

---

## Acknowledgments

This project relies entirely on the foundational reverse-engineering work of the Linux hardware community:

- **[Linuwu-Sense](https://github.com/0x7375646F/Linuwu-Sense)** by [@0x7375646F](https://github.com/0x7375646F) — The kernel module that exposes Acer Predator hardware controls via sysfs. Massive thanks for reverse-engineering the Acer WMI interface.
- **[ph16-71-rgb](https://github.com/Order52/ph16-71-rgb)** by [@Order52](https://github.com/Order52) — The original Python implementation of the USB HID RGB protocol for the Acer Predator PH16-71 keyboard. Huge thanks for documenting the packet structure.

---

## License

MIT — see [LICENSE](LICENSE) for details.
