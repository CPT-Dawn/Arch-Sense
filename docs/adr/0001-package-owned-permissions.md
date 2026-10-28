# Permissions are shipped by the package, not installed by the app

Arch-Sense runs unprivileged. Instead of a self-installer that uses pkexec to create groups, udev rules and a systemd unit at runtime, the AUR package ships static `sysusers.d` and udev files that grant an `arch-sense` group access to the Linuwu-Sense controls and the USB keyboard. pacman owns every file, so upgrades and uninstalls are clean; the only manual step is adding the user to the group.

## Considered Options

- **pkexec self-installer** (v1): worked, but left untracked files on the system and duplicated logic pacman already does.
- **Privileged daemon**: more moving parts than a handful of sysfs writes justifies.
