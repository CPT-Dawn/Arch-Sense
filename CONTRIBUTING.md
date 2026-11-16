# Contributing

Bug reports, ideas and pull requests are all welcome. Start with an [issue](https://github.com/CPT-Dawn/Arch-Sense/issues/new/choose), and paste your `arch-sense doctor` output if something is broken.

## Setup

You need Rust (stable, edition 2024) and `libusb`. No Acer laptop required:

```sh
git clone https://github.com/CPT-Dawn/Arch-Sense && cd Arch-Sense
cargo run --release -- --demo        # the full app on a simulated laptop
```

`--demo` and `arch-sense doctor` are safe anywhere. The plain TUI and `--apply` write to real hardware.

## Checks

CI ([`.github/workflows/check.yml`](.github/workflows/check.yml)) runs these on every push and pull request; run them before you open one:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
packaging/check.sh            # both AUR packages; add --build for makepkg
```

Tests run against fake hardware trees (`src/fixture`) and a recording keyboard, so they never touch your machine.

## Where things live

| Path | What's there |
| --- | --- |
| `src/` | The app |
| [GLOSSARY.md](GLOSSARY.md) | The project's vocabulary (Capability, Lighting Backend, Saved Lighting…) |
| [docs/adr](docs/adr) | The bigger design decisions and why |
| [packaging/aur](packaging/aur) | The AUR packages |
| [demo/arch-sense.tape](demo/arch-sense.tape) | The README demo |

## Re-recording the demo

The demo is recorded with [VHS](https://github.com/charmbracelet/vhs) on the simulated laptop. From the repo root:

```sh
cargo build --release
vhs demo/arch-sense.tape
ffmpeg -i assets/demo.mp4 -vf "fps=20,scale='min(2560,iw)':-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=256:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" -loop 0 assets/demo.gif
```

Keep the GIF under 8 MB.

## Support a new keyboard

Some keyboards can't be driven yet; the Jing-Mold `05af:666a` in the Predator PH16-72 is the known example. Adding one means learning how the keyboard is spoken to, and that takes **USB captures** from someone who owns it:

1. Run `lsusb` and `arch-sense doctor` and note the keyboard's `vendor:product` ID.
2. On Windows, install [Wireshark](https://www.wireshark.org/) with USBPcap and start capturing on the keyboard's USB hub.
3. In Acer's own software (PredatorSense / NitroSense), change one thing at a time: a static colour, then brightness, then each effect, speed and direction. Write down what you changed and when.
4. Open an issue with the `lsusb` line, the Doctor output, the capture files (`.pcapng`) and your notes.

That's how the USB HID support came to be: [ph16-71-rgb](https://github.com/Order52/ph16-71-rgb) documented the PH16-71's packets, and Arch-Sense speaks them.

## Releasing

Bump `version` in `Cargo.toml`, run `cargo update -p arch-sense`, bump `pkgver` in `packaging/aur/arch-sense/PKGBUILD`, and add the release to [CHANGELOG.md](CHANGELOG.md). Then tag `vX.Y.Z` and publish a GitHub release; [`release.yml`](.github/workflows/release.yml) pushes the AUR package.
