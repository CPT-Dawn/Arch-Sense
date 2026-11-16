# Installation

How to install Arch-Sense from the AUR, turn on Boot Restore, upgrade from v1 and uninstall. The package recipe lives in [`packaging/aur/arch-sense/PKGBUILD`](../packaging/aur/arch-sense/PKGBUILD).

## Install

Arch-Sense is on the [AUR](https://aur.archlinux.org/packages/arch-sense). It pulls in [`linuwu-sense-dkms-git`](https://aur.archlinux.org/packages/linuwu-sense-dkms-git), which builds the kernel module for every kernel you have installed.

1. **Install the package.** You also need the headers for your kernel (for example `linux-headers`).

   ```sh
   paru -S arch-sense
   ```

   On CachyOS or another clang-built kernel, install `clang lld llvm` first. See [the module won't build](troubleshooting.md#the-module-wont-build).

2. **Join the `arch-sense` group.** This lets Arch-Sense drive the hardware without sudo.

   ```sh
   sudo usermod -aG arch-sense $USER
   ```

3. **Reboot.** Group changes only reach new logins, and the reboot also swaps the built-in `acer_wmi` driver for `linuwu_sense`. If the module was already running, logging out and back in is enough.

4. **Run it.**

   ```sh
   arch-sense
   ```

Every file belongs to pacman, so there is nothing else to set up and nothing left behind when you [uninstall](#uninstall).

## Restore lighting at boot

Keyboards forget their lighting when the power goes. **Boot Restore** reapplies your Saved Lighting at every boot, before you log in. It is off until you turn it on:

```sh
sudo systemctl enable arch-sense.service
```

It only touches lighting; Linuwu-Sense and the firmware already remember everything else. The service runs `arch-sense --apply`.

## Upgrading from v1

Your keyboard lighting carries over automatically on first run. Three things changed.

**1. Permissions are the package's job now.** Join the group as in [step 2](#install), then remove the files v1 installed by hand (the Doctor and the package's install message point them out too):

```sh
sudo systemctl disable arch-sense-permissions.service
sudo rm -f /etc/udev/rules.d/70-arch-sense.rules /etc/systemd/system/arch-sense-permissions.service
```

**2. Boot Restore is opt-in.** v1 switched it on for you; the upgrade switches it off again. To get lighting back at boot, [enable it](#restore-lighting-at-boot).

**3. The kernel module comes from the AUR.** If you built Linuwu-Sense by hand with `make install`, remove that copy first so the two don't fight. From your Linuwu-Sense checkout:

```sh
make uninstall
```

No checkout any more? Check `dkms status` for your version, then remove it by hand:

```sh
sudo systemctl disable --now linuwu_sense.service
sudo dkms remove linuwu_sense/1.0 --all
sudo rm -rf /usr/src/linuwu_sense-1.0
sudo rm -f /etc/systemd/system/linuwu_sense.service /etc/modules-load.d/linuwu_sense.conf \
  /etc/modprobe.d/blacklist-acer_wmi.conf /etc/tmpfiles.d/linuwu_sense.conf
```

## Uninstall

```sh
paru -Rns arch-sense linuwu-sense-dkms-git
```

pacman removes every file it installed. Two things stay on purpose, in case you come back: your Saved Lighting in `/var/lib/arch-sense`, and the `arch-sense` group (pacman never deletes groups). To remove those too:

```sh
sudo rm -r /var/lib/arch-sense
sudo groupdel arch-sense
```

[Back to the README](../README.md)
