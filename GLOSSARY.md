# Arch-Sense

A terminal control center for Acer Predator and Nitro laptops, built on top of the Linuwu-Sense kernel module.

## Platform

**Linuwu-Sense**:
The third-party kernel module that exposes Acer gaming-laptop hardware controls to Linux. Arch-Sense is a frontend for it.
_Avoid_: the driver, acer-wmi, the module

**Sense Interface**:
The family of controls Linuwu-Sense exposes for a laptop line: Predator Sense or Nitro Sense. A laptop has exactly one.
_Avoid_: sense group, sysfs folder, mode

**Capability**:
One hardware control a specific laptop actually exposes, such as the battery limiter or fan speed. Arch-Sense shows only the capabilities present.
_Avoid_: feature, setting

**Thermal Profile**:
The firmware power/thermal mode (e.g. Quiet, Balanced, Performance).
_Avoid_: power mode, performance mode, fan profile

## Keyboard lighting

**Lighting Backend**:
The mechanism that drives a keyboard's RGB. There are two: **Four-Zone** (via Linuwu-Sense) and **USB HID** (talking to the keyboard controller directly).
_Avoid_: RGB driver, keyboard mode

**Lighting Effect**:
An animation or static pattern the keyboard renders on its own, like Wave or Breathing.
_Avoid_: RGB mode, animation

**Saved Lighting**:
The last lighting configuration the user applied, persisted so it can be restored.
_Avoid_: config, preset

**Boot Restore**:
Reapplying the Saved Lighting at system boot, before login. Opt-in.

## Support

**Doctor**:
The built-in diagnosis that explains why a laptop isn't fully working and what to do about it.
_Avoid_: health check, debug mode
