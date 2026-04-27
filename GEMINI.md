# Arch-Sense Project Context

## Project Overview

**Arch-Sense** is a professional, high-performance hardware control center for Acer Predator laptops running Arch Linux. It replaces the Windows-only "PredatorSense" utility.

### Core Technologies

- **Language:** Rust (2021 Edition)
- **TUI Framework:** `ratatui` with `crossterm` backend
- **CLI Framework:** `clap` (v4 with derive API)
- **Hardware I/O:**
  - `rusb` (libusb) for per-key RGB lighting via HID protocol
  - `sysfs` for thermal profiles, fan speeds, and battery management (requires `linuwu_sense` kernel module)
- **Architecture:** Modular and thread-safe. A dedicated hardware worker thread manages blocking I/O (USB/sysfs) and communicates with the main TUI thread via `mpsc` channels.

### System Components

- **`src/main.rs`**: Entry point and CLI argument dispatch.
- **`src/cli.rs`**: Declarative CLI definition using `clap`.
- **`src/commands.rs`**: Headless CLI command implementations.
- **`src/app.rs`**: Main TUI state machine, event loop, and animation logic.
- **`src/hardware.rs`**: Background worker loop and low-level hardware drivers.
- **`src/permissions.rs`**: Udev, polkit (pkexec), and systemd integration for rootless access.
- **`src/ui.rs`**: Declarative UI rendering layer using `ratatui`.

## Building and Running

### Development Commands

- **Build:** `cargo build`
- **Release Build:** `cargo build --release`
- **Run TUI:** `cargo run`
- **Run Tests:** `cargo test`
- **Linting:** `cargo clippy -- -D warnings`
- **Formatting:** `cargo fmt`

### Production Commands

- **Check Status:** `arch-sense --doctor`
- **Install Permissions:** `arch-sense --install-permissions`
- **Apply Saved RGB:** `arch-sense --apply`
- **Service Management:** `sudo systemctl enable --now arch-sense` (applies settings on boot)

## Development Conventions

### Coding Standards

- **Error Handling:** Strictly use `anyhow::Result` for error propagation. Avoid `unwrap()` and `expect()` in library modules; use `unwrap_or` or `?` with context.
- **Performance:**
  - Prefer borrowing (`&T`) over cloning where possible.
  - `RgbSettings` and small data structs should implement `Copy`.
  - Minimize allocations within the `ui.rs` rendering loop (use `.into_iter()` on fixed arrays).
- **Concurrency:** Hardware I/O must never block the main thread. Always use the worker pattern in `hardware.rs`.
- **Types:** Use Newtypes and Enums (e.g., `ControlId`, `RgbField`) instead of raw strings or magic numbers for hardware attributes.

### Testing Practices

- **Unit Tests:** Located at the bottom of modules (e.g., `models.rs`, `hardware.rs`) within `#[cfg(test)]` blocks.
- **Mocking:** Hardware snapshots are boxed and sent via channels, allowing for theoretical decoupling of UI testing from hardware presence.

### Documentation

- Use `///` for public-facing documentation and `//` for implementation details.
- Complex hardware protocols (like the RGB packet structure in `hardware.rs`) must be explicitly documented with comments.
