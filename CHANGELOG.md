# Changelog

What changed in each release. The full notes for every version are on [GitHub Releases](https://github.com/CPT-Dawn/Arch-Sense/releases).

## 2.0.2 (2026-11-09)

Bug fixes; no config, CLI or file-location changes.

- **Sensors:** the CPU card shows the Package temperature, not a random core; AMD GPUs show `edge`, not VRAM (#23). Ryzen laptops with an NVIDIA card show the dGPU (#24). A lone GPU fan is no longer shown as the CPU fan (#25). A hung `nvidia-smi` can no longer freeze the app (#32).
- **Dashboard:** battery calibration needs a real `y` (#27). Lighting errors say why (#26). `NO_COLOR` keeps the Fans gauge readable (#28). The Battery card keeps its watt unit at 80 columns (#30). "# custom" stays visible and clickable at 110–115 columns (#35).
- **Terminal:** SIGTERM, SIGINT and SIGHUP quit cleanly and restore the terminal; `--demo` cleans up its temp folder (#31).
- **Doctor:** Permissions checks the controls are writable (#33). Scrolling back up works after overscrolling (#34). An Acer with no DMI model name is no longer called unsupported (#29).

## 2.0.1 (2026-10-28)

- `arch-sense --demo`: the Doctor examines the simulated laptop, not your machine.
- The README gained a demo GIF, a quick start and troubleshooting entries.

## 2.0.0 (2026-10-28)

A rewrite around one idea: if Linuwu-Sense supports your laptop, Arch-Sense does too.

- **Every Linuwu-Sense laptop:** detects the Sense Interface and shows only the Capabilities that work.
- **Two Lighting Backends:** Four-Zone keyboards via Linuwu-Sense, and the Chicony `04f2:0117` USB keyboard (PH16-71 family) driven directly.
- **New dashboard:** sensor cards on top, one settings list below, live apply, mouse support, help overlay, 16-colour and `NO_COLOR` fallback.
- **`arch-sense doctor`** and **`arch-sense --demo`**.
- **Packaged properly:** a sysusers group and udev rules for permissions, and `linuwu-sense-dkms-git` for the kernel module.

**Breaking:** permissions need the `arch-sense` group (pkexec is gone); Boot Restore is opt-in; fans are Auto or Max; there is no Enter-to-apply step; the kernel module comes from the AUR. See [Upgrading from v1](docs/installation.md#upgrading-from-v1).

## 1.x

- **1.3.0–1.3.2 (2026-04-28):** TUI update and better permission handling.
- **1.2.0 (2026-03-21):** RGB settings persist automatically when applied.
- **1.1.0 (2026-03-17):** library-first refactor; binary at `/usr/bin/arch-sense`, state in `/var/lib/arch-sense`.
- **1.0.0 (2026-02-23):** first release.
