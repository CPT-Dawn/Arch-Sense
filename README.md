<h1 align="center">◆ A R C H - S E N S E ◆</h1>
<h3 align="center">The terminal control center for Acer Predator &amp; Nitro laptops</h3>

<p align="center">
  Thermal profiles, fans, battery care and keyboard lighting for every laptop
  <a href="https://github.com/0x7375646F/Linuwu-Sense">Linuwu-Sense</a> supports.<br>
  One package, no sudo, no Windows. PredatorSense, but it fits in a terminal.
</p>

<p align="center">
  <a href="https://aur.archlinux.org/packages/arch-sense"><img alt="AUR" src="https://img.shields.io/aur/version/arch-sense?style=flat-square&logo=archlinux&logoColor=white&color=1793d1"></a>
  <a href="https://github.com/CPT-Dawn/Arch-Sense/actions/workflows/check.yml"><img alt="Check" src="https://img.shields.io/github/actions/workflow/status/CPT-Dawn/Arch-Sense/check.yml?style=flat-square&label=check"></a>
  <img alt="Acer Predator & Nitro" src="https://img.shields.io/badge/acer-predator_%26_nitro-39ff14?style=flat-square">
  <img alt="Rust 2024" src="https://img.shields.io/badge/rust-2024_edition-orange?style=flat-square&logo=rust">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-green?style=flat-square"></a>
</p>

<p align="center">
  <img alt="Arch-Sense running on the simulated demo laptop: gauges, settings, keyboard lighting, help and the Doctor" src="docs/demo.gif" width="900">
</p>

## Quick start

```bash
paru -S arch-sense                      # 1. install (pulls in the kernel module)
sudo usermod -aG arch-sense $USER       # 2. allow yourself to use it without sudo
reboot                                  # 3. reboot, then run: arch-sense
```

Just curious? `arch-sense --demo` runs the whole app on a simulated laptop and
touches nothing on your machine. That's what the GIF above shows.

## Contents

- [What it does](#what-it-does)
- [Does my laptop work?](#does-my-laptop-work)
- [Install](#install)
- [Using Arch-Sense](#using-arch-sense)
- [Troubleshooting](#troubleshooting)
- [Upgrading from v1](#upgrading-from-v1)
- [Uninstall](#uninstall)
- [Contributing](#contributing)
- [Acknowledgments](#acknowledgments) · [License](#license)

---

## What it does

Arch-Sense shows what your laptop is up to and lets you change what it can:

| | |
|---|---|
| **Watch** | CPU and GPU temperatures, fan speeds, battery level and power draw, live |
| **Performance** | Thermal Profile (e.g. Quiet, Balanced, Performance), fans on Auto or Max |
| **Battery** | 80% charge limit, USB charging while off, battery calibration |
| **System** | Boot animation, LCD overdrive, keyboard backlight timeout |
| **Lighting** | Effects, colours (palette, custom hex or per zone), brightness, speed, direction |
| **Diagnose** | A built-in Doctor that finds what's wrong and tells you the fix |

It only shows the controls **your** laptop actually has, so nothing on screen is
a decoy. Changes apply instantly, and your lighting is saved and restored for you.

---

## Does my laptop work?

**If [Linuwu-Sense](https://github.com/0x7375646F/Linuwu-Sense) supports your laptop,
Arch-Sense does too.** Arch-Sense is a frontend for that kernel module, so check its
README for your model first.

Everything except keyboard lighting works the same on every supported laptop.
Lighting depends on which keyboard you have:

| Your keyboard | Works? | What you get |
|---|---|---|
| **Four-zone RGB** (many Nitro and older Predator models) | ✅ Yes | Static colours (all zones or one at a time), Breathing, Neon, Wave, Shifting, Zoom, Meteor, Twinkling |
| **Chicony `04f2:0117`** (Predator PH16-71 family) | ✅ Yes | Static, Breathing, Wave, Snake, Ripple, Rainbow, Rain, Lightning, Spot, Stars, Fireball, Snow, Heartbeat |
| **Jing-Mold `05af:666a`** (Predator PH16-72) | ❌ Not yet | The app says so honestly. You can [help add it](#help-support-a-new-keyboard) |
| **ENE KB5130** (some 2025+ models) | ⚠️ No | The keyboard accepts commands and ignores them. A [known upstream limitation](https://github.com/0x7375646F/Linuwu-Sense#note-for-2025-models-enek5130-keyboards); the Doctor warns you |

Not sure which keyboard you have? Install Arch-Sense and run `arch-sense doctor`.
The *Lighting Backend* line tells you.

---

## Install

Arch-Sense is on the [AUR](https://aur.archlinux.org/packages/arch-sense). It pulls in
[`linuwu-sense-dkms-git`](https://aur.archlinux.org/packages/linuwu-sense-dkms-git),
which builds the kernel module for every kernel you have installed.

**1. Install the package.** You also need the headers for your kernel (e.g.
`linux-headers`) if you don't have them yet.

```bash
paru -S arch-sense
```

> On CachyOS or another clang-built kernel? Install `clang lld llvm` first. See
> [the module won't build](#the-module-wont-build-cachyos-or-another-clang-built-kernel).

**2. Join the `arch-sense` group.** This is what lets Arch-Sense drive the hardware
without sudo.

```bash
sudo usermod -aG arch-sense $USER
```

**3. Reboot.** Group changes only reach new logins, and the reboot also swaps the
built-in `acer_wmi` driver for `linuwu_sense`. (If the module was already running,
logging out and back in is enough.)

**4. Run it.**

```bash
arch-sense
```

That's it. Every file belongs to pacman, so there's nothing else to set up and nothing
left behind when you [uninstall](#uninstall).

### Optional: restore lighting at boot

Keyboards forget their lighting when the power goes. **Boot Restore** reapplies your
saved lighting at every boot, before you log in. It's off until you turn it on:

```bash
sudo systemctl enable arch-sense.service
```

It only touches lighting; Linuwu-Sense and the firmware already remember everything else.

---

## Using Arch-Sense

### The dashboard

- **Top:** gauge cards for CPU, GPU, Fans and Battery.
- **Below:** one settings list, grouped into **Performance**, **Battery**, **System**
  and **Lighting**. In Lighting, a live keyboard preview appears at the bottom.
- **Footer:** confirms each change, or tells you why it didn't work.

There's no "Apply" button: changes take effect the moment you make them. The only
thing that asks first is battery calibration.

### Keys

| Key | Action |
|---|---|
| `↑` `↓` or `k` `j` | Move between settings |
| `←` `→` or `h` `l` | Change the selected setting |
| `Enter` / `Space` | Toggle, run, or step forward |
| `Tab` / `Shift-Tab` | Jump between sections |
| `#` | Type a custom hex colour (on the Colour row) |
| `?` | Help |
| `d` | Doctor |
| `Esc` | Close any dialog |
| `q` / `Ctrl-C` | Quit |

**Mouse:** click a row to select it, click an option or swatch to set it, click the
selected row to step it forward (right-click steps back), and scroll to move.

Needs a terminal of at least **80×24**. Any smaller and it will politely ask for more room.

### Commands

| Command | What it does |
|---|---|
| `arch-sense` | Open the dashboard |
| `arch-sense doctor` | Check your setup and explain how to fix anything broken |
| `arch-sense --demo` | Try the full app on a simulated laptop; touches nothing real |
| `arch-sense --apply` | Reapply saved lighting without opening the app (what Boot Restore runs) |
| `arch-sense --help` | Show all options |

---

## Troubleshooting

### Start with the Doctor

```bash
arch-sense doctor
```

The **Doctor** checks the kernel module, DKMS, your kernel's compiler, Secure Boot,
`acer_wmi`, your laptop's controls, your group membership, the keyboard lighting
and Boot Restore. Anything that isn't right comes with the exact command that fixes it:

```text
✓ Laptop            Acer Predator PH16-71
✓ Kernel module     linuwu_sense loaded
✓ DKMS              built for 6.18.2-arch1-1
✓ Permissions       in the arch-sense group
✓ Lighting Backend  USB HID (04f2:0117)
i Boot Restore      off (optional)
                    → To reapply Saved Lighting at boot: `sudo systemctl enable arch-sense.service`
```

It's also one keypress away inside the app: press `d`. Its output is plain text, made
for pasting into a [bug report](https://github.com/CPT-Dawn/Arch-Sense/issues/new).

### Common problems

Click a problem to expand it.

<details>
<summary><b>The module won't build (CachyOS or another clang-built kernel)</b></summary>

<a id="the-module-wont-build-cachyos-or-another-clang-built-kernel"></a>

Kernels built with clang need the module built with clang too.
`linuwu-sense-dkms-git` handles that by itself, as long as the LLVM toolchain is
installed. Install it, then reinstall the module so DKMS builds it again:

```bash
paru -S clang lld llvm
paru -S linuwu-sense-dkms-git
```

`arch-sense doctor` shows which compiler your kernel was built with.

</details>

<details>
<summary><b>I use Secure Boot and the module is "missing"</b></summary>

Secure Boot refuses to load unsigned modules. DKMS signs the modules it builds with its
own key (`/var/lib/dkms/mok.key`, configured in `/etc/dkms/framework.conf`); your
firmware just needs to trust it. If you boot through shim, enroll the key:

```bash
sudo mokutil --import /var/lib/dkms/mok.pub
```

Reboot and confirm the enrollment in the blue MokManager screen. If you manage your
own keys (for example with `sbctl`), sign with those instead by pointing
`mok_signing_key` and `mok_certificate` in `/etc/dkms/framework.conf` at them.

</details>

<details>
<summary><b>My PH16-71 loads the module but Arch-Sense finds no controls</b></summary>

Upstream Linuwu-Sense briefly dropped the PH16-71 from its model list, so the module
loads without creating its controls. The fix is pending upstream
([Linuwu-Sense#152](https://github.com/0x7375646F/Linuwu-Sense/pull/152)), and
`linuwu-sense-dkms-git` already carries it, so updating is enough:

```bash
paru -S linuwu-sense-dkms-git
```

If you built the module some other way, tell it which model it is talking to and
reload it:

```bash
echo 'options linuwu_sense predator_v4=1' | sudo tee /etc/modprobe.d/linuwu-sense-ph16-71.conf
sudo modprobe -r linuwu_sense && sudo modprobe linuwu_sense
```

Delete that file once upstream has the fix. `arch-sense doctor` spots this case.

</details>

<details>
<summary><b>It says I don't have permission</b></summary>

Make sure you joined the group (step 2 of [Install](#install)) and then logged out
and back in or rebooted. Group changes don't reach sessions that were already
running. `arch-sense doctor` tells you which of the two is missing.

</details>

Still stuck? [Open an issue](https://github.com/CPT-Dawn/Arch-Sense/issues/new) and
paste your `arch-sense doctor` output.

---

## Upgrading from v1

Mostly nothing to do: your keyboard lighting carries over automatically on first run.
Three things changed.

**1. Permissions are the package's job now.** Join the group as in
[Install, step 2](#install), then remove the files v1 installed by hand (the Doctor
and the package's install message point them out too):

```bash
sudo systemctl disable arch-sense-permissions.service
sudo rm -f /etc/udev/rules.d/70-arch-sense.rules /etc/systemd/system/arch-sense-permissions.service
```

**2. Boot Restore is opt-in.** v1 switched it on for you; the upgrade switches it off
again. If you want lighting back at boot, [enable it](#optional-restore-lighting-at-boot).

**3. The kernel module comes from the AUR.** If you built Linuwu-Sense by hand with
`make install`, remove that copy first so the two don't fight. From your Linuwu-Sense
checkout:

```bash
make uninstall
```

<details>
<summary>No checkout any more? Remove it manually</summary>

Check `dkms status` for your version, then:

```bash
sudo systemctl disable --now linuwu_sense.service
sudo dkms remove linuwu_sense/1.0 --all
sudo rm -rf /usr/src/linuwu_sense-1.0
sudo rm -f /etc/systemd/system/linuwu_sense.service /etc/modules-load.d/linuwu_sense.conf \
  /etc/modprobe.d/blacklist-acer_wmi.conf /etc/tmpfiles.d/linuwu_sense.conf
```

</details>

---

## Uninstall

```bash
paru -Rns arch-sense linuwu-sense-dkms-git
```

pacman removes every file it installed. Two things stay on purpose, in case you come
back: your saved lighting in `/var/lib/arch-sense`, and the `arch-sense` group (pacman
never deletes groups). To remove those too:

```bash
sudo rm -r /var/lib/arch-sense
sudo groupdel arch-sense
```

---

## Contributing

Bug reports, ideas and pull requests are all welcome. Start with an
[issue](https://github.com/CPT-Dawn/Arch-Sense/issues), and paste your
`arch-sense doctor` output if something's broken.

### Help support a new keyboard

Some keyboards can't be driven yet; the Jing-Mold `05af:666a` in the Predator PH16-72
is the known example. Adding one means learning how the keyboard is spoken to, and
that takes **USB captures** from someone who owns it:

1. Run `lsusb` and `arch-sense doctor` and note the keyboard's `vendor:product` ID.
2. On Windows, install [Wireshark](https://www.wireshark.org/) with USBPcap and start
   capturing on the keyboard's USB hub.
3. In Acer's own software (PredatorSense / NitroSense), change one thing at a time:
   a static colour, then brightness, then each effect, speed and direction. Write down
   what you changed and when.
4. Open an issue with the `lsusb` line, the Doctor output, the capture files (`.pcapng`)
   and your notes.

That's how the USB HID support came to be: [ph16-71-rgb](https://github.com/Order52/ph16-71-rgb)
documented the PH16-71's packets, and Arch-Sense speaks them.

### Hacking on Arch-Sense

You need Rust (stable, edition 2024) and `libusb`. No Acer laptop required:

```bash
cargo run -- --demo                          # run it on the simulated laptop
cargo fmt --check                            # formatting
cargo clippy --all-targets -- -D warnings    # lints
cargo test                                   # tests
```

Where things live:

| Path | What's there |
|---|---|
| `src/` | The app. Tests run against fake hardware trees (`src/fixture`) and a recording keyboard, so they never touch your machine |
| [GLOSSARY.md](GLOSSARY.md) | The project's vocabulary (Capability, Lighting Backend, Saved Lighting…) |
| [docs/adr](docs/adr) | The bigger design decisions and why |
| [packaging/aur](packaging/aur) | The AUR packages |
| [docs/demo.tape](docs/demo.tape) | Records the GIF above with [VHS](https://github.com/charmbracelet/vhs): `cargo build --release && vhs docs/demo.tape` |

---

## Acknowledgments

Arch-Sense stands on the reverse-engineering work of others:

- **[Linuwu-Sense](https://github.com/0x7375646F/Linuwu-Sense)** by [@0x7375646F](https://github.com/0x7375646F): the kernel module that exposes Acer's gaming-laptop controls on Linux. Arch-Sense would be a very pretty screensaver without it.
- **[ph16-71-rgb](https://github.com/Order52/ph16-71-rgb)** by [@Order52](https://github.com/Order52): the original Python implementation of the PH16-71 keyboard's USB HID protocol.

## License

MIT. See [LICENSE](LICENSE).
