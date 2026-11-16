# Troubleshooting

The problems people hit, each with its fix. Start with the Doctor; its checks live in [`src/doctor.rs`](../src/doctor.rs).

## Start with the Doctor

```sh
arch-sense doctor
```

The Doctor checks the kernel module, DKMS, your kernel's compiler, Secure Boot, `acer_wmi`, your laptop's controls, your permissions, the keyboard lighting, Boot Restore and v1 leftovers. Anything that isn't right comes with the exact command that fixes it:

```text
✓ Laptop            Acer Predator PH16-71
✓ Kernel module     linuwu_sense loaded
✓ DKMS              built for 6.18.2-arch1-1
✓ Permissions       in the arch-sense group
✓ Lighting Backend  USB HID (04f2:0117)
i Boot Restore      off (optional)
                    → To reapply Saved Lighting at boot: `sudo systemctl enable arch-sense.service`
```

It is also one keypress away inside the app: press <kbd>d</kbd>. Its output is plain text, made for pasting into a [bug report](https://github.com/CPT-Dawn/Arch-Sense/issues/new/choose).

## The module won't build

This happens on CachyOS and other clang-built kernels. They need the module built with clang too. `linuwu-sense-dkms-git` handles that by itself, as long as the LLVM toolchain is installed. Install it, then reinstall the module so DKMS builds it again:

```sh
paru -S clang lld llvm
paru -S linuwu-sense-dkms-git
```

`arch-sense doctor` shows which compiler your kernel was built with.

## Secure Boot says the module is missing

Secure Boot refuses to load unsigned modules. DKMS signs the modules it builds with its own key (`/var/lib/dkms/mok.key`, configured in `/etc/dkms/framework.conf`); your firmware just needs to trust it. If you boot through shim, enroll the key:

```sh
sudo mokutil --import /var/lib/dkms/mok.pub
```

Reboot and confirm the enrollment in the blue MokManager screen. If you manage your own keys (for example with `sbctl`), point `mok_signing_key` and `mok_certificate` in `/etc/dkms/framework.conf` at them instead.

## My PH16-71 loads the module but shows no controls

Upstream Linuwu-Sense briefly dropped the PH16-71 from its model list, so the module loads without creating its controls. The fix is pending upstream ([Linuwu-Sense#152](https://github.com/0x7375646F/Linuwu-Sense/pull/152)), and `linuwu-sense-dkms-git` already carries it, so updating is enough:

```sh
paru -S linuwu-sense-dkms-git
```

If you built the module some other way, tell it which model it is talking to and reload it:

```sh
echo 'options linuwu_sense predator_v4=1' | sudo tee /etc/modprobe.d/linuwu-sense-ph16-71.conf
sudo modprobe -r linuwu_sense && sudo modprobe linuwu_sense
```

Delete that file once upstream has the fix. `arch-sense doctor` spots this case.

## It says I don't have permission

Make sure you joined the group ([Installation, step 2](installation.md#install)) and then logged out and back in or rebooted. Group changes don't reach sessions that were already running. `arch-sense doctor` tells you which of the two is missing.

## Still stuck?

[Open an issue](https://github.com/CPT-Dawn/Arch-Sense/issues/new/choose) and paste your `arch-sense doctor` output.

[Back to the README](../README.md)
