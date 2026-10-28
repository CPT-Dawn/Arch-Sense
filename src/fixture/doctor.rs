//! Fixture pieces the Doctor looks at: the kernel, DKMS, Secure Boot, conflicting
//! modules, keyboards and the installed systemd units.

use super::FakeMachineBuilder;
use crate::machine::MachineRoot;

/// The kernel release fake machines run unless told otherwise.
const KERNEL_RELEASE: &str = "6.18.2-arch1-1";

impl FakeMachineBuilder {
    /// A PH16-71 with everything set up the way the package intends: module
    /// built by DKMS and loaded, USB HID keyboard, Boot Restore enabled.
    pub fn healthy_ph16_71(self) -> Self {
        self.predator_sense()
            .acer("Predator PH16-71")
            .gcc_kernel()
            .dkms_built()
            .secure_boot(false)
            .usb_hid_keyboard()
            .boot_restore()
    }

    /// An Acer laptop of model `model` according to DMI.
    pub fn acer(self, model: &str) -> Self {
        self.file(MachineRoot::dmi_sys_vendor, "Acer\n")
            .product_name(model)
    }

    /// The running kernel, built with gcc (headers installed).
    pub fn gcc_kernel(self) -> Self {
        self.kernel(
            "gcc (GCC) 15.2.1, GNU ld (GNU Binutils) 2.45",
            "CONFIG_CC_IS_GCC=y\n",
        )
    }

    /// The running kernel, built with clang the way CachyOS builds it.
    pub fn clang_kernel(self) -> Self {
        self.kernel(
            "clang version 22.1.3, LLD 22.1.3",
            "CONFIG_CC_IS_CLANG=y\nCONFIG_LD_IS_LLD=y\n",
        )
    }

    fn kernel(self, compiler: &str, config: &'static str) -> Self {
        let banner = format!("Linux version {KERNEL_RELEASE} (builder@host) ({compiler}) #1 SMP\n");
        self.file(MachineRoot::kernel_release, format!("{KERNEL_RELEASE}\n"))
            .file(MachineRoot::proc_version, banner)
            .file(
                |m| m.kernel_modules_dir(KERNEL_RELEASE).join("build/.config"),
                config,
            )
    }

    /// `linuwu-sense-dkms-git` installed, but DKMS hasn't built it for the
    /// running kernel.
    pub fn dkms_registered(self) -> Self {
        self.file(
            |m| m.dkms_tree().join("linuwu-sense/1.0.r42/source/dkms.conf"),
            "PACKAGE_NAME=\"linuwu-sense\"\n",
        )
    }

    /// `linuwu-sense-dkms-git` installed and built for the running kernel.
    pub fn dkms_built(self) -> Self {
        self.dkms_registered().file(
            |m| {
                m.dkms_tree()
                    .join("linuwu-sense/1.0.r42")
                    .join(KERNEL_RELEASE)
                    .join("x86_64/module/linuwu_sense.ko.zst")
            },
            "",
        )
    }

    /// The in-kernel `acer_wmi` driver loaded alongside (or instead of) Linuwu-Sense.
    pub fn acer_wmi_loaded(self) -> Self {
        self.file(|m| m.sys_module("acer_wmi").join("refcnt"), "0\n")
    }

    /// The EFI Secure Boot variable: 4 attribute bytes, then 1 when it's on.
    pub fn secure_boot(self, on: bool) -> Self {
        let value = if on {
            "\u{6}\0\0\0\u{1}"
        } else {
            "\u{6}\0\0\0\0"
        };
        self.file(MachineRoot::secure_boot_var, value)
    }

    /// Linuwu-Sense's Four-Zone keyboard lighting folder.
    pub fn four_zoned_kb(self) -> Self {
        self.file(
            |m| m.four_zone_kb_dir().join("four_zone_mode"),
            "0,0,100,0,255,255,255\n",
        )
        .file(
            |m| m.four_zone_kb_dir().join("per_zone_mode"),
            "ffffff,ffffff,ffffff,ffffff,100\n",
        )
    }

    /// The ENE KB5130 keyboard of 2025+ models (e.g. PHN16S-71), whose LEDs
    /// ignore the Four-Zone writes Linuwu-Sense accepts.
    pub fn ene_kb5130(self) -> Self {
        self.file(
            |m| m.hidraw_dir().join("hidraw2/device/uevent"),
            "DRIVER=hid-generic\nHID_ID=0018:00000CF2:00005130\nHID_NAME=ENEK5130:00 0CF2:5130\n",
        )
    }

    /// A USB device `vendor:product`, e.g. a keyboard Arch-Sense can't drive yet.
    pub fn usb_device(self, vendor: u16, product: u16) -> Self {
        let device = |m: &MachineRoot| m.usb_devices_dir().join("1-7");
        self.file(
            move |m| device(m).join("idVendor"),
            format!("{vendor:04x}\n"),
        )
        .file(
            move |m| device(m).join("idProduct"),
            format!("{product:04x}\n"),
        )
    }

    /// The PH16-72's Jing-Mold keyboard (`05af:666a`), which no backend drives yet.
    pub fn jing_mold_keyboard(self) -> Self {
        self.usb_device(0x05af, 0x666a)
    }

    /// Boot Restore enabled with `systemctl enable arch-sense.service`.
    pub fn boot_restore(self) -> Self {
        self.file(
            MachineRoot::boot_restore_link,
            "[Service]\nExecStart=/usr/bin/arch-sense --apply\n",
        )
    }

    /// The files v1's pkexec self-installer left behind.
    pub fn v1_leftovers(self) -> Self {
        self.file(MachineRoot::udev_rule, "# v1\n")
            .file(MachineRoot::permission_service, "[Unit]\n")
            .file(MachineRoot::permission_service_link, "[Unit]\n")
    }

    /// The `arch-sense` group (gid 966) with `members`, and user `dawn` (uid
    /// 1000) whose current process holds `session_groups`.
    pub fn login(self, members: &[&str], session_groups: &[u32]) -> Self {
        let groups: Vec<String> = session_groups.iter().map(u32::to_string).collect();
        let status = format!(
            "Name:\tarch-sense\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\nGroups:\t{}\n",
            groups.join(" ")
        );
        self.file(MachineRoot::proc_self_status, status)
            .file(
                MachineRoot::etc_passwd,
                "root:x:0:0::/root:/bin/bash\ndawn:x:1000:1000::/home/dawn:/bin/zsh\n",
            )
            .file(
                MachineRoot::etc_group,
                format!(
                    "root:x:0:root\nwheel:x:998:dawn\narch-sense:x:966:{}\ndawn:x:1000:\n",
                    members.join(",")
                ),
            )
    }
}
