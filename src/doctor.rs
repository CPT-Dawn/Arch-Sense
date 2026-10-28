//! The Doctor: explains what isn't working on this laptop and how to fix it.
//!
//! [`diagnose`] is a pure function of the [`MachineRoot`] and the user's
//! [`Membership`] of the `arch-sense` group. It runs no commands: what `dkms
//! status` or `systemctl is-enabled` would say is read from the files they read.
//! The CLI (`arch-sense doctor`) prints the [`Report`]; the TUI shows the same
//! checks in an overlay.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::capabilities::{self, SenseInterface};
use crate::constants::JOIN_GROUP;
use crate::keyboard::{USB_HID_PRODUCT_ID, USB_HID_VENDOR_ID, UsbAccess, keyboard_access};
use crate::lighting::{self, CONTRIBUTING_URL, Detection, LightingBackend};
use crate::machine::MachineRoot;

const MODULE: &str = "linuwu_sense";
const GROUP: &str = "arch-sense";
/// The names DKMS may know Linuwu-Sense by: our package's and upstream's `make install`.
const DKMS_NAMES: [&str; 2] = ["linuwu-sense", "linuwu_sense"];
/// Where a `linuwu_sense.ko` may be installed for a kernel, relative to its modules folder.
const MODULE_LOCATIONS: [&str; 4] = [
    "updates/dkms",
    "updates",
    "extra",
    "kernel/drivers/platform/x86",
];
/// The ENE KB5130 keyboard of 2025+ models, as hidraw names it.
const ENE_KB5130_ID: &str = "00000CF2:00005130";
const NEW_ISSUE: &str = "https://github.com/CPT-Dawn/Arch-Sense/issues/new";
const UPSTREAM_ISSUES: &str = "https://github.com/0x7375646F/Linuwu-Sense/issues";
const ENE_NOTE: &str =
    "https://github.com/0x7375646F/Linuwu-Sense#note-for-2025-models-enek5130-keyboards";

/// How a check came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ok,
    /// Fine as it is, but there's something optional to do.
    Info,
    /// Works, partly or with a caveat.
    Warn,
    /// Broken: `arch-sense doctor` exits non-zero.
    Error,
}

impl Status {
    /// The checklist mark, plain enough to survive being pasted into an issue.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Ok => "✓",
            Self::Info => "i",
            Self::Warn => "!",
            Self::Error => "✗",
        }
    }
}

/// One line of the checklist. Every check that isn't [`Status::Ok`] has a remedy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    /// What was found, in a few words.
    pub detail: String,
    /// What to do about it: a command or a step.
    pub remedy: Option<String>,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Ok,
            detail: detail.into(),
            remedy: None,
        }
    }

    fn not_ok(
        name: &'static str,
        status: Status,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
            remedy: Some(remedy.into()),
        }
    }
}

/// Everything the Doctor found, in checklist order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    checks: Vec<Check>,
}

impl Report {
    pub fn checks(&self) -> &[Check] {
        &self.checks
    }

    /// Whether any check failed outright; the CLI exits non-zero exactly then.
    pub fn has_errors(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == Status::Error)
    }

    /// The process exit code for `arch-sense doctor`.
    pub fn exit_code(&self) -> i32 {
        i32::from(self.has_errors())
    }
}

/// The checklist, one check per line, remedies indented underneath.
impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self.checks.iter().map(|c| c.name.len()).max().unwrap_or(0);
        for check in &self.checks {
            writeln!(
                f,
                "{} {:width$}  {}",
                check.status.mark(),
                check.name,
                check.detail
            )?;
            if let Some(remedy) = &check.remedy {
                writeln!(f, "  {:width$}  → {remedy}", "")?;
            }
        }
        Ok(())
    }
}

/// The current user's place in the `arch-sense` group, which grants hardware access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Membership {
    /// In the group, and this login session has it.
    Member,
    /// Added to the group, but this session started before that: log out and in.
    StaleLogin,
    /// Not in the group.
    NotMember,
    /// The group doesn't exist (the package's sysusers file hasn't run).
    NoGroup,
    /// Running as root, which needs no group.
    Root,
}

impl Membership {
    /// Works out the current process's membership from `/proc/self/status`,
    /// `/etc/passwd` and `/etc/group` under `machine`.
    pub fn current(machine: &MachineRoot) -> Self {
        let status = read(machine.proc_self_status()).unwrap_or_default();
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .map(|rest| {
                    rest.split_whitespace()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let uid = field("Uid:").into_iter().next();
        if uid.as_deref() == Some("0") {
            return Self::Root;
        }
        let mut session_groups = field("Groups:");
        session_groups.extend(field("Gid:").into_iter().next());

        let groups = read(machine.etc_group()).unwrap_or_default();
        let Some((gid, members)) = groups.lines().find_map(|line| {
            let mut parts = line.split(':');
            (parts.next()? == GROUP).then(|| {
                let gid = parts.nth(1).unwrap_or_default().to_owned();
                let members: Vec<String> = parts
                    .next()
                    .unwrap_or_default()
                    .split(',')
                    .map(str::to_owned)
                    .collect();
                (gid, members)
            })
        }) else {
            return Self::NoGroup;
        };

        if session_groups.contains(&gid) {
            return Self::Member;
        }
        let passwd = read(machine.etc_passwd()).unwrap_or_default();
        let user = passwd.lines().find_map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            (parts.len() > 3 && Some(parts[2]) == uid.as_deref())
                .then(|| (parts[0].to_owned(), parts[3].to_owned()))
        });
        match user {
            Some((name, primary)) if members.contains(&name) || primary == gid => Self::StaleLogin,
            _ => Self::NotMember,
        }
    }
}

/// Examines `machine` and returns the checklist.
pub fn diagnose(machine: &MachineRoot, membership: Membership) -> Report {
    let facts = Facts::gather(machine);
    let checks = vec![
        laptop(machine),
        kernel_module(&facts),
        dkms(&facts),
        kernel_compiler(&facts),
        secure_boot(machine, &facts),
        acer_wmi(&facts),
        sense_interface(machine, &facts),
        permissions(membership),
        lighting(machine),
        boot_restore(machine),
        v1_leftovers(machine),
    ];
    Report { checks }
}

/// What several checks need to know.
struct Facts {
    loaded: bool,
    acer_wmi: bool,
    release: Option<String>,
    clang: Option<bool>,
    headers: bool,
    build: Build,
}

/// How Linuwu-Sense is built for the running kernel.
enum Build {
    /// DKMS has built it.
    Dkms,
    /// A module file is installed, but not by DKMS (e.g. upstream `make install`).
    Manual,
    /// DKMS has the source but no build for this kernel.
    DkmsNotBuilt { log: String },
    /// Nothing installed.
    Missing,
}

impl Facts {
    fn gather(machine: &MachineRoot) -> Self {
        let release = read(machine.kernel_release()).map(|r| r.trim().to_owned());
        let build_dir = release
            .as_deref()
            .map(|release| machine.kernel_modules_dir(release).join("build"));
        let clang = build_dir
            .as_ref()
            .and_then(|dir| read(dir.join(".config")))
            .map(|config| config.lines().any(|l| l == "CONFIG_CC_IS_CLANG=y"))
            .or_else(|| read(machine.proc_version()).map(|v| v.contains("clang")));
        Self {
            loaded: machine.sys_module(MODULE).is_dir(),
            acer_wmi: machine.sys_module("acer_wmi").is_dir(),
            headers: build_dir.is_some_and(|dir| dir.is_dir()),
            build: release
                .as_deref()
                .map_or(Build::Missing, |release| build(machine, release)),
            release,
            clang,
        }
    }

    fn release(&self) -> &str {
        self.release.as_deref().unwrap_or("the running kernel")
    }
}

fn build(machine: &MachineRoot, release: &str) -> Build {
    let mut registered = None;
    for name in DKMS_NAMES {
        for version in subdirs(&machine.dkms_tree().join(name)) {
            if !version.join("source").exists() && !version.join(release).exists() {
                continue;
            }
            let built = subdirs(&version.join(release))
                .iter()
                .any(|arch| has_module(&arch.join("module")));
            if built {
                return Build::Dkms;
            }
            registered.get_or_insert_with(|| shown(machine, &version.join("build/make.log")));
        }
    }
    let modules = machine.kernel_modules_dir(release);
    if MODULE_LOCATIONS
        .iter()
        .any(|location| has_module(&modules.join(location)))
    {
        return Build::Manual;
    }
    match registered {
        Some(log) => Build::DkmsNotBuilt { log },
        None => Build::Missing,
    }
}

fn has_module(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("linuwu_sense.ko")
        })
    })
}

fn laptop(machine: &MachineRoot) -> Check {
    const NAME: &str = "Laptop";
    let vendor = read(machine.dmi_sys_vendor()).map(|v| v.trim().to_owned());
    let model = read(machine.dmi_product_name()).map(|m| m.trim().to_owned());
    match (vendor, model) {
        (Some(vendor), Some(model)) if vendor.eq_ignore_ascii_case("acer") => {
            Check::ok(NAME, format!("{vendor} {model}"))
        }
        (Some(vendor), model) => Check::not_ok(
            NAME,
            Status::Error,
            format!("{vendor} {}", model.unwrap_or_default())
                .trim_end()
                .to_owned(),
            "Arch-Sense only drives Acer Predator and Nitro laptops that Linuwu-Sense supports",
        ),
        (None, _) => Check::not_ok(
            NAME,
            Status::Warn,
            "model unknown (no DMI information)",
            "Make sure /sys/class/dmi/id is readable; mention your exact model when asking for help",
        ),
    }
}

fn kernel_module(facts: &Facts) -> Check {
    const NAME: &str = "Kernel module";
    if facts.loaded {
        return Check::ok(NAME, "linuwu_sense loaded");
    }
    let remedy = match facts.build {
        Build::Dkms | Build::Manual => {
            "Load it: `sudo modprobe linuwu_sense` (it loads at boot once installed)".to_owned()
        }
        Build::DkmsNotBuilt { .. } | Build::Missing => {
            "It isn't built for this kernel; see the DKMS check below".to_owned()
        }
    };
    Check::not_ok(NAME, Status::Error, "linuwu_sense isn't loaded", remedy)
}

fn dkms(facts: &Facts) -> Check {
    const NAME: &str = "DKMS";
    let release = facts.release();
    let headers = format!(
        "Install the headers for {release} (e.g. `sudo pacman -S linux-headers`), then `sudo dkms autoinstall`"
    );
    match &facts.build {
        Build::Dkms => Check::ok(NAME, format!("built for {release}")),
        Build::Manual => Check::not_ok(
            NAME,
            Status::Info,
            "installed by hand, not managed by DKMS",
            "It won't be rebuilt when the kernel updates; `paru -S linuwu-sense-dkms-git` takes care of that",
        ),
        Build::DkmsNotBuilt { .. } if !facts.headers => Check::not_ok(
            NAME,
            Status::Error,
            format!("not built for {release}: kernel headers missing"),
            headers,
        ),
        Build::DkmsNotBuilt { log } => Check::not_ok(
            NAME,
            Status::Error,
            format!("not built for {release}"),
            format!("Run `sudo dkms autoinstall`; if it fails, the build log is {log}"),
        ),
        Build::Missing => Check::not_ok(
            NAME,
            Status::Error,
            "Linuwu-Sense isn't installed",
            "Install it: `paru -S linuwu-sense-dkms-git`",
        ),
    }
}

fn kernel_compiler(facts: &Facts) -> Check {
    const NAME: &str = "Kernel compiler";
    match facts.clang {
        Some(true) if facts.loaded => Check::ok(NAME, "clang (module built with LLVM)"),
        Some(true) => Check::not_ok(
            NAME,
            Status::Warn,
            "clang-built kernel: the module must be built with LLVM too",
            "`sudo pacman -S clang lld llvm`, then `sudo dkms autoinstall` (DKMS adds LLVM=1 itself)",
        ),
        Some(false) => Check::ok(NAME, "gcc"),
        None => Check::ok(NAME, "unknown (no sign of clang)"),
    }
}

fn secure_boot(machine: &MachineRoot, facts: &Facts) -> Check {
    const NAME: &str = "Secure Boot";
    let on = fs::read(machine.secure_boot_var())
        .ok()
        .and_then(|bytes| bytes.last().copied())
        == Some(1);
    match (on, facts.loaded) {
        (false, _) => Check::ok(NAME, "off"),
        (true, true) => Check::ok(NAME, "on, module signed and loaded"),
        (true, false) => Check::not_ok(
            NAME,
            Status::Warn,
            "on: an unsigned module won't load",
            secure_boot_remedy(),
        ),
    }
}

/// How to sign the module. The commands run on the real machine, so they name
/// its paths even when the Doctor is looking at a fake one.
fn secure_boot_remedy() -> String {
    let system = MachineRoot::system();
    let conf = system.dkms_framework_conf();
    let cert = system.dkms_mok_cert();
    format!(
        "Sign it: set mok_signing_key/mok_certificate in {} (DKMS's default key is {}), `sudo mokutil --import {}`, reboot and enrol the key",
        conf.display(),
        cert.display(),
        cert.display()
    )
}

fn acer_wmi(facts: &Facts) -> Check {
    const NAME: &str = "acer_wmi";
    if !facts.acer_wmi {
        return Check::ok(NAME, "not loaded");
    }
    Check::not_ok(
        NAME,
        Status::Error,
        "loaded: it claims the same WMI interface as Linuwu-Sense",
        "`sudo modprobe -r acer_wmi && sudo modprobe linuwu_sense`; linuwu-sense-dkms-git blacklists it, so a reboot also fixes it",
    )
}

fn sense_interface(machine: &MachineRoot, facts: &Facts) -> Check {
    const NAME: &str = "Sense Interface";
    let laptop = capabilities::detect(machine);
    let count = laptop.capabilities().len();
    let detail = |interface: &str| format!("{interface}, {count} Capabilities");
    match laptop.sense_interface {
        SenseInterface::Predator => Check::ok(NAME, detail("Predator Sense")),
        SenseInterface::Nitro => Check::ok(NAME, detail("Nitro Sense")),
        SenseInterface::None if !facts.loaded => Check::not_ok(
            NAME,
            Status::Error,
            "not found",
            "Fix the kernel module first",
        ),
        SenseInterface::None => Check::not_ok(
            NAME,
            Status::Error,
            "not found: Linuwu-Sense doesn't recognise this model",
            format!(
                "Try `sudo modprobe -r linuwu_sense && sudo modprobe linuwu_sense predator_v4=1` (or nitro_v4=1), and report your model at {UPSTREAM_ISSUES}"
            ),
        ),
    }
}

fn permissions(membership: Membership) -> Check {
    const NAME: &str = "Permissions";
    match membership {
        Membership::Member => Check::ok(NAME, "in the arch-sense group"),
        Membership::Root => Check::ok(NAME, "running as root"),
        Membership::StaleLogin => Check::not_ok(
            NAME,
            Status::Error,
            "added to arch-sense, but this login predates it",
            "Log out and back in (or reboot); `newgrp arch-sense` works for one shell",
        ),
        Membership::NotMember => Check::not_ok(
            NAME,
            Status::Error,
            "not in the arch-sense group",
            JOIN_GROUP,
        ),
        Membership::NoGroup => Check::not_ok(
            NAME,
            Status::Error,
            "the arch-sense group doesn't exist",
            format!(
                "Install the package (`paru -S arch-sense`) or run `sudo systemd-sysusers`, then {JOIN_GROUP}"
            ),
        ),
    }
}

fn lighting(machine: &MachineRoot) -> Check {
    const NAME: &str = "Lighting Backend";
    if has_ene_kb5130(machine) {
        return Check::not_ok(
            NAME,
            Status::Warn,
            "ENE KB5130 keyboard: lighting writes are accepted but the LEDs ignore them",
            format!("A known limitation of 2025+ models, not your setup; see {ENE_NOTE}"),
        );
    }
    match lighting::detect(machine) {
        Detection::Backend(LightingBackend::FourZone) => Check::ok(NAME, "Four-Zone"),
        Detection::Backend(LightingBackend::UsbHid) => usb_hid_lighting(machine),
        Detection::Unsupported(keyboard) => Check::not_ok(
            NAME,
            Status::Warn,
            format!("{keyboard} keyboard isn't supported yet"),
            format!(
                "Help add it: share USB captures of the vendor software changing colours, as described at {CONTRIBUTING_URL} (or open an issue at {NEW_ISSUE})"
            ),
        ),
        Detection::None => Check::not_ok(
            NAME,
            Status::Info,
            "no RGB keyboard found",
            format!(
                "If your keyboard has RGB lighting, open an issue at {NEW_ISSUE} with your `lsusb` output"
            ),
        ),
    }
}

/// The USB HID backend is only as good as our access to the keyboard.
fn usb_hid_lighting(machine: &MachineRoot) -> Check {
    const NAME: &str = "Lighting Backend";
    match keyboard_access(machine) {
        UsbAccess::Accessible | UsbAccess::NotFound => Check::ok(
            NAME,
            format!("USB HID ({USB_HID_VENDOR_ID:04x}:{USB_HID_PRODUCT_ID:04x})"),
        ),
        UsbAccess::PermissionDenied => Check::not_ok(
            NAME,
            Status::Warn,
            "USB HID keyboard found, but it can't be opened",
            "Fix Permissions above, then `sudo udevadm trigger` (or reboot)",
        ),
        UsbAccess::Error(error) => Check::not_ok(
            NAME,
            Status::Warn,
            format!("USB HID keyboard: {error}"),
            format!("Report it at {NEW_ISSUE} with this output"),
        ),
    }
}

fn has_ene_kb5130(machine: &MachineRoot) -> bool {
    subdirs(&machine.hidraw_dir()).iter().any(|device| {
        read(device.join("device/uevent")).is_some_and(|uevent| {
            uevent.to_ascii_uppercase().contains(ENE_KB5130_ID) || uevent.contains("ENEK5130")
        })
    })
}

fn boot_restore(machine: &MachineRoot) -> Check {
    const NAME: &str = "Boot Restore";
    if exists(&machine.boot_restore_link()) {
        return Check::ok(NAME, "enabled");
    }
    Check::not_ok(
        NAME,
        Status::Info,
        "off (optional)",
        "To reapply Saved Lighting at boot: `sudo systemctl enable arch-sense.service`",
    )
}

fn v1_leftovers(machine: &MachineRoot) -> Check {
    const NAME: &str = "v1 leftovers";
    let files: Vec<PathBuf> = [
        machine.udev_rule(),
        machine.permission_service(),
        machine.permission_service_link(),
    ]
    .into_iter()
    .filter(|path| exists(path))
    .collect();
    if files.is_empty() {
        return Check::ok(NAME, "none");
    }
    let mut remedy = String::new();
    if exists(&machine.permission_service_link()) {
        remedy.push_str("`sudo systemctl disable arch-sense-permissions.service`, then ");
    }
    let removable: Vec<String> = [machine.udev_rule(), machine.permission_service()]
        .iter()
        .filter(|path| exists(path))
        .map(|path| shown(machine, path))
        .collect();
    if !removable.is_empty() {
        remedy.push_str(&format!("`sudo rm {}`", removable.join(" ")));
    }
    Check::not_ok(
        NAME,
        Status::Warn,
        "v1's self-installed permission files are still there (the package replaces them)",
        remedy.trim_end_matches(", then ").to_owned(),
    )
}

/// `path` as it reads on the real machine: the part under the machine root.
fn shown(machine: &MachineRoot, path: &Path) -> String {
    match path.strip_prefix(machine.root()) {
        Ok(relative) => format!("/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Whether `path` exists, counting a dangling symlink (an enabled unit whose
/// target is gone is still enabled).
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

fn read(path: PathBuf) -> Option<String> {
    fs::read_to_string(path).ok()
}
