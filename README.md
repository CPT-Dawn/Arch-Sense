# Arch-Sense

Arch-Sense is a Rust + kernel module stack for controlling Acer Predator/Nitro laptop features from a local daemon and TUI client.

## Components

- `kernel/`: `linuwu_sense` kernel module exposing Predator/Nitro controls through sysfs.
- `daemon/`: Unix socket control plane (`/tmp/arch-sense.sock`) and persistent config manager.
- `client/`: Terminal dashboard for telemetry and control.
- `shared/`: IPC command/response types used by client and daemon.

## New RGB Features

- RGB animation speed control (`1..10`)
- RGB brightness control (`0..100`)
- Expanded animation set:
	- `neon`, `wave`, `breath`, `rainbow`, `reactive`
	- `ripple`, `starlight`, `rain`, `fire`, `aurora`
- Exact live values shown in TUI for:
	- active RGB mode, speed, brightness
	- fan mode and system toggles

## TUI Keybindings

- Fans: `a` Auto, `b` Balanced, `t` Turbo
- RGB Colors: `1` Red, `2` Green, `3` Blue, `4` White, `5` Pink
- RGB Effects: `n` next, `p` previous, `x` apply selected
- RGB Speed: `+` increase, `-` decrease
- RGB Brightness: `]` increase, `[` decrease
- System: `l` battery limiter, `c` calibration, `o` LCD overdrive, `m` boot animation, `k` backlight timeout, `u` USB threshold cycle
- Quit: `q`