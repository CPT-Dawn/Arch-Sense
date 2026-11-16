# Supported hardware

Which laptops and keyboards Arch-Sense drives. Detection lives in [`src/capabilities.rs`](../src/capabilities.rs) and the keyboard backends in [`src/lighting/`](../src/lighting).

## Does my laptop work?

**If [Linuwu-Sense](https://github.com/0x7375646F/Linuwu-Sense) supports your laptop, Arch-Sense does too.** Arch-Sense is a frontend for that kernel module, so check its README for your model first.

Arch-Sense detects your laptop's Sense Interface (Predator Sense or Nitro Sense) and shows only the Capabilities it actually has:

| Section | Controls |
| --- | --- |
| Watch | CPU and GPU temperatures, fan speeds, battery level and power draw, live |
| Performance | Thermal Profile (for example Quiet, Balanced, Performance), fans on Auto or Max |
| Battery | 80% charge limit, USB charging while off, battery calibration |
| System | Boot animation, LCD overdrive, keyboard backlight timeout |
| Lighting | Effects, colours (palette, custom hex or per zone), brightness, speed, direction |

## Keyboards

Everything except keyboard lighting works the same on every supported laptop. Lighting depends on the keyboard:

| Keyboard | Works? | What you get |
| --- | --- | --- |
| **Four-zone RGB** (many Nitro and older Predator models) | ✅ Yes | Static colours (all zones or one at a time), Breathing, Neon, Wave, Shifting, Zoom, Meteor, Twinkling |
| **Chicony `04f2:0117`** (Predator PH16-71 family) | ✅ Yes | Static, Breathing, Wave, Snake, Ripple, Rainbow, Rain, Lightning, Spot, Stars, Fireball, Snow, Heartbeat |
| **Jing-Mold `05af:666a`** (Predator PH16-72) | ❌ Not yet | The app says so. You can [help add it](../CONTRIBUTING.md#support-a-new-keyboard) |
| **ENE KB5130** (some 2025+ models) | ⚠️ No | The keyboard accepts commands and ignores them, a [known upstream limitation](https://github.com/0x7375646F/Linuwu-Sense#note-for-2025-models-enek5130-keyboards); the Doctor warns you |

Not sure which keyboard you have? Run `arch-sense doctor`: the *Lighting Backend* line tells you.

## GPU temperature

Arch-Sense reads temperatures from hwmon first. When hwmon has no GPU sensor, it falls back to `nvidia-smi` from the optional `nvidia-utils` package.

[Back to the README](../README.md)
