# Permissions are shipped by the package, not installed by the app

Arch-Sense runs unprivileged. Instead of a self-installer that uses pkexec to create groups, udev rules and a systemd unit at runtime, the AUR package ships static `sysusers.d` and udev files that grant an `arch-sense` group access to the Linuwu-Sense controls and the USB keyboard. pacman owns every file, so upgrades and uninstalls are clean; the only manual step is adding the user to the group.

## Considered Options

- **pkexec self-installer** (v1): worked, but left untracked files on the system and duplicated logic pacman already does.
- **Privileged daemon**: more moving parts than a handful of sysfs writes justifies.

## Consequences

- **Uninstall keeps the Saved Lighting and the `arch-sense` group.** pacman never deletes groups (other packages or files may still use the GID), and `/var/lib/arch-sense` holds the user's data, so it is only removed when empty. Following that convention, `post_remove` prints the two commands that remove them for anyone who wants a clean slate.
- **The install script does a little more than reload udev.** pacman's sysusers hook runs after the install script, but udev can only hand out a group that exists, so the script runs `systemd-sysusers arch-sense.conf` before reloading and re-triggering udev. That way permissions work straight after `paru -S`, without a reboot.
