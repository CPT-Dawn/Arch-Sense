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
  <img alt="Arch-Sense running in demo mode" src="docs/demo.gif" width="900">
</p>

Arch-Sense shows what your laptop is up to (CPU and GPU temperatures, fan speeds,
battery) and lets you change what it can: Thermal Profile, fans, charge limit, USB
charging, boot animation, LCD overdrive and keyboard lighting. It only shows the
controls your laptop actually has, so nothing on screen is a decoy.

Want a look before installing anything? `arch-sense --demo` runs the whole UI on a
simulated laptop and touches nothing real. That's what the GIF above is.

---

## Does my laptop work?

Short answer: **if Linuwu-Sense supports your laptop, Arch-Sense does too.** Arch-Sense
is a frontend for the [Linuwu-Sense](https://github.com/0x7375646F/Linuwu-Sense) kernel
module, so check its README for your model first.

Arch-Sense detects your laptop's **Sense Interface** (Predator Sense or Nitro Sense) and
shows the **Capabilities** it finds there. Controls your firmware exposes but rejects
are hidden, not shown broken.

Keyboard lighting depends on the keyboard, through one of two **Lighting Backends**:

| Your keyboard | Lighting Backend | What you get |
|---|---|---|
| Four-zone RGB (many Nitro and older Predator models) | **Four-Zone**, via Linuwu-Sense | Static colours per zone (all zones or one at a time), plus Breathing, Neon, Wave, Shifting, Zoom, Meteor and Twinkling |
| Chicony `04f2:0117` (Predator PH16-71 family) | **USB HID**, talking to the keyboard directly | Static, Breathing, Wave, Snake, Ripple, Rainbow, Rain, Lightning, Spot, Stars, Fireball, Snow and Heartbeat |
| Jing-Mold `05af:666a` (Predator PH16-72) | Not supported yet | An honest "not yet" in the app, and [a way to help](#contributing) |
| ENE KB5130 (some 2025+ models) | Four-Zone, in theory | The keyboard accepts the writes and ignores them. A [known limitation](https://github.com/0x7375646F/Linuwu-Sense#note-for-2025-models-enek5130-keyboards) upstream; the Doctor will warn you |

Not sure which keyboard you have? Install Arch-Sense and run `arch-sense doctor`; the
*Lighting Backend* line will tell you.

---

## Install in three steps

Arch-Sense is on the AUR. It pulls in
[`linuwu-sense-dkms-git`](https://aur.archlinux.org/packages/linuwu-sense-dkms-git), which
builds the kernel module for every kernel you have installed.

**1. Install the package** (plus the headers for your kernel, e.g. `linux-headers`, if
you don't have them yet):

```bash
paru -S arch-sense
```

**2. Join the `arch-sense` group**, which is what lets Arch-Sense drive the hardware without sudo:

```bash
sudo usermod -aG arch-sense $USER
```

**3. Reboot.** Group changes only reach new logins, and the reboot also swaps the
in-kernel `acer_wmi` driver for `linuwu_sense`. (If the module was already running,
logging out and back in is enough.)

That's it. Every file belongs to pacman, so there's nothing else to set up and nothing
left behind when you uninstall.

### Optional: Boot Restore

Keyboards forget their lighting when the power goes. **Boot Restore** reapplies your
Saved Lighting at every boot, before you log in. It's opt-in:

```bash
sudo systemctl enable arch-sense.service
```

It only touches lighting. Linuwu-Sense and the firmware already remember everything else.

---

## First run

```bash
arch-sense
```

Across the top are gauge cards for CPU, GPU, Fans and Battery. Below them is one
settings list, grouped into **Performance**, **Battery**, **System** and **Lighting**.
Every value is shown inline, and changes apply the moment you make them: no "apply"
button, no pending state. The footer confirms each change (or tells you why it didn't
work).

Your lighting is saved automatically as **Saved Lighting** and restored next time.

| Keys | What they do |
|---|---|
| `↑` `↓` or `k` `j` | Select the previous or next setting |
| `←` `→` or `h` `l` | Change the selected setting |
| `Enter` / `Space` | Toggle, run, or step forward |
| `Tab` / `Shift-Tab` | Jump between sections |
| `#` | Type a custom hex colour (on Colour) |
| `?` | Show or hide the help |
| `d` | Open the Doctor |
| `Esc` | Close any dialog |
| `q` / `Ctrl-C` | Quit |

The mouse works too: click a row to select it, click an option or swatch to set it,
click the selected row to step it forward (right-click steps back), and scroll to move.

Arch-Sense needs a terminal of at least 80×24. Smaller than that and it will politely
ask for more room.

---

## Something's off? Run `arch-sense doctor`

Before anything else:

```bash
arch-sense doctor
```

The **Doctor** checks the kernel module, DKMS, your kernel's compiler, Secure Boot,
`acer_wmi`, the Sense Interface, your group membership, the Lighting Backend and Boot
Restore. Anything that isn't right comes with the exact command or step that fixes it.
A trimmed example:

```text
✓ Laptop            Acer Predator PH16-71
✓ Kernel module     linuwu_sense loaded
✓ DKMS              built for 6.18.2-arch1-1
✓ Permissions       in the arch-sense group
✓ Lighting Backend  USB HID (04f2:0117)
i Boot Restore      off (optional)
                    → To reapply Saved Lighting at boot: `sudo systemctl enable arch-sense.service`
```

It exits non-zero when something is broken, and its output is plain text made for
pasting into a [bug report](https://github.com/CPT-Dawn/Arch-Sense/issues/new). The same
checks are one keypress away inside the app: press `d`.

---

## FAQ

### I'm on CachyOS (or another clang-built kernel) and the module won't build

Kernels built with clang need the module built with clang too, or the build bails out
over the compiler mismatch. `linuwu-sense-dkms-git` handles that by itself, as long as
the LLVM toolchain is installed:

```bash
paru -S clang lld llvm
paru -S linuwu-sense-dkms-git
```

Reinstalling the module package makes DKMS build it again, this time with clang.
`arch-sense doctor` shows which compiler your kernel was built with.

### I use Secure Boot and the module is "missing"

Secure Boot refuses to load unsigned modules. DKMS signs the modules it builds with its
own key (`/var/lib/dkms/mok.key`, configured in `/etc/dkms/framework.conf`); your
firmware just needs to trust it. If you boot through shim, enroll the key:

```bash
sudo mokutil --import /var/lib/dkms/mok.pub
```

Reboot and confirm the enrollment in the blue MokManager screen. If you manage your
own keys (for example with `sbctl`), sign with those instead by pointing
`mok_signing_key` and `mok_certificate` in `/etc/dkms/framework.conf` at them.

### My PH16-71 loads the module but Arch-Sense finds no controls

Upstream Linuwu-Sense briefly dropped the PH16-71 from its model list, so the module
loads without creating its `predator_sense` controls. The fix is pending upstream
([Linuwu-Sense#152](https://github.com/0x7375646F/Linuwu-Sense/pull/152)), and
`linuwu-sense-dkms-git` already carries it, so updating is enough:
`paru -S linuwu-sense-dkms-git`. If you built the module some other way, tell it which
model it is talking to and reload it:

```bash
echo 'options linuwu_sense predator_v4=1' | sudo tee /etc/modprobe.d/linuwu-sense-ph16-71.conf
sudo modprobe -r linuwu_sense && sudo modprobe linuwu_sense
```

Delete that file once upstream has the fix. `arch-sense doctor` spots this case.

### How do I uninstall it?

```bash
paru -Rns arch-sense linuwu-sense-dkms-git
```

pacman removes every file it installed. Two things stay on purpose, in case you want
them back: your Saved Lighting in `/var/lib/arch-sense`, and the `arch-sense` group
(pacman never deletes groups). To remove those too:

```bash
sudo rm -r /var/lib/arch-sense
sudo groupdel arch-sense
```

### I'm upgrading from v1. Anything to do?

Mostly no. Your keyboard lighting carries over to Saved Lighting automatically on first
run. Three things changed:

- **Permissions are the package's job now.** v1's self-installing permission
  setup is gone. Join the group as in [step 2](#install-in-three-steps), then remove
  the files v1 installed by hand (the Doctor and the package's install message point them out too):

  ```bash
  sudo systemctl disable arch-sense-permissions.service
  sudo rm -f /etc/udev/rules.d/70-arch-sense.rules /etc/systemd/system/arch-sense-permissions.service
  ```

- **Boot Restore is opt-in.** v1 switched it on for you; the upgrade switches it off
  again. If you relied on lighting coming back at boot, enable it as
  [shown above](#optional-boot-restore).
- **The kernel module comes from the AUR.** If you built Linuwu-Sense by hand with
  `make install`, Arch-Sense still works with it, but you no longer need to maintain it
  yourself.

---

## Contributing

Bug reports, ideas and pull requests are all welcome. Start with an
[issue](https://github.com/CPT-Dawn/Arch-Sense/issues), and paste your
`arch-sense doctor` output if something's broken.

### Help support a new keyboard

Arch-Sense can't drive every keyboard yet. The Jing-Mold `05af:666a` in the Predator
PH16-72 is the known example. Adding one means learning how the keyboard is spoken to,
and that takes **USB captures** from someone who owns it:

1. Run `lsusb` and `arch-sense doctor` and note the keyboard's `vendor:product` ID.
2. On Windows, install [Wireshark](https://www.wireshark.org/) with USBPcap and start
   capturing on the keyboard's USB hub.
3. In Acer's own software (PredatorSense / NitroSense), change one thing at a time:
   a static colour, then brightness, then each effect, speed and direction. Write down
   what you changed and when.
4. Open an issue with the `lsusb` line, the Doctor output, the capture files (`.pcapng`)
   and your notes.

That's how the USB HID backend came to be: [ph16-71-rgb](https://github.com/Order52/ph16-71-rgb)
documented the PH16-71's packets, and Arch-Sense speaks them.

### Hacking on Arch-Sense

You need Rust (stable, edition 2024) and `libusb`. No Acer laptop required:

```bash
cargo run -- --demo
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests run against fake hardware trees (`src/fixture`) and a recording keyboard, so they
never touch your machine. The vocabulary lives in [GLOSSARY.md](GLOSSARY.md) and the
bigger decisions in [docs/adr](docs/adr). The AUR packages are in
[packaging/aur](packaging/aur).

The demo GIF is recorded with [VHS](https://github.com/charmbracelet/vhs) from
[docs/demo.tape](docs/demo.tape): `cargo build --release && vhs docs/demo.tape`.

---

## Acknowledgments

Arch-Sense stands on the reverse-engineering work of others:

- **[Linuwu-Sense](https://github.com/0x7375646F/Linuwu-Sense)** by [@0x7375646F](https://github.com/0x7375646F): the kernel module that exposes Acer's gaming-laptop controls on Linux. Arch-Sense would be a very pretty screensaver without it.
- **[ph16-71-rgb](https://github.com/Order52/ph16-71-rgb)** by [@Order52](https://github.com/Order52): the original Python implementation of the PH16-71 keyboard's USB HID protocol.

## License

MIT. See [LICENSE](LICENSE).
