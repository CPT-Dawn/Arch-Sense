use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use rusb::{DeviceHandle, Error as UsbError, GlobalContext};

use crate::config::{config_dir, config_path};
use crate::constants::{ps, KB_PID, KB_VID, PLATFORM_PROFILE};

pub(crate) const HARDWARE_GROUP: &str = "arch-sense";

const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/70-arch-sense.rules";
const PERMISSION_SERVICE_PATH: &str = "/etc/systemd/system/arch-sense-permissions.service";
const INSTALLED_BINARY_PATH: &str = "/usr/bin/arch-sense";
const ROOT_INSTALL_FLAG: &str = "--install-permissions-root";

const SYSFS_ATTRS: &[&str] = &[
    "backlight_timeout",
    "battery_calibration",
    "battery_limiter",
    "boot_animation_sound",
    "fan_speed",
    "lcd_override",
    "usb_charging",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UsbAccess {
    Accessible,
    PermissionDenied,
    NotFound,
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KeyboardOpenError {
    PermissionDenied,
    NotFound,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PathAccess {
    Writable,
    Missing,
    PermissionDenied,
    Error(String),
}

#[derive(Clone, Debug)]
pub(crate) struct PermissionReport {
    pub(crate) is_root: bool,
    pub(crate) sysfs: Vec<(PathBuf, PathAccess)>,
    pub(crate) usb: UsbAccess,
}

impl PermissionReport {
    pub(crate) fn collect() -> Self {
        Self {
            is_root: is_root(),
            sysfs: sysfs_write_paths()
                .into_iter()
                .map(|path| {
                    let access = path_write_access(&path);
                    (path, access)
                })
                .collect(),
            usb: keyboard_access(),
        }
    }

    pub(crate) fn has_limited_access(&self) -> bool {
        if self.is_root {
            return false;
        }

        let sysfs_denied = self.sysfs.iter().any(|(_, access)| {
            matches!(access, PathAccess::PermissionDenied | PathAccess::Error(_))
        });
        sysfs_denied || matches!(self.usb, UsbAccess::PermissionDenied)
    }

    pub(crate) fn startup_hint(&self) -> Option<String> {
        if self.has_limited_access() {
            Some(format!("Limited access - {}", setup_hint()))
        } else {
            None
        }
    }
}

pub(crate) fn setup_hint() -> &'static str {
    "run `arch-sense --install-permissions` once, then log out and back in if prompted"
}

pub(crate) fn keyboard_present() -> bool {
    !matches!(keyboard_access(), UsbAccess::NotFound)
}

pub(crate) fn keyboard_access() -> UsbAccess {
    match try_open_keyboard() {
        Ok(_) => UsbAccess::Accessible,
        Err(KeyboardOpenError::PermissionDenied) => UsbAccess::PermissionDenied,
        Err(KeyboardOpenError::NotFound) => UsbAccess::NotFound,
        Err(KeyboardOpenError::Other(err)) => UsbAccess::Error(err),
    }
}

pub(crate) fn open_keyboard() -> Result<DeviceHandle<GlobalContext>> {
    match try_open_keyboard() {
        Ok(handle) => Ok(handle),
        Err(KeyboardOpenError::PermissionDenied) => bail!(
            "Keyboard USB access denied (VID:04F2 PID:0117); {}",
            setup_hint()
        ),
        Err(KeyboardOpenError::NotFound) => bail!("Keyboard not found (VID:04F2 PID:0117)"),
        Err(KeyboardOpenError::Other(err)) => {
            bail!("Keyboard found but could not be opened: {err}")
        }
    }
}

fn try_open_keyboard() -> std::result::Result<DeviceHandle<GlobalContext>, KeyboardOpenError> {
    let devices = rusb::devices().map_err(|e| KeyboardOpenError::Other(e.to_string()))?;
    let mut found = false;
    let mut access_denied = false;
    let mut last_error = None;

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(desc) => desc,
            Err(err) => {
                last_error = Some(err.to_string());
                continue;
            }
        };

        if desc.vendor_id() != KB_VID || desc.product_id() != KB_PID {
            continue;
        }

        found = true;
        match device.open() {
            Ok(handle) => return Ok(handle),
            Err(UsbError::Access) => access_denied = true,
            Err(err) => last_error = Some(err.to_string()),
        }
    }

    if access_denied {
        Err(KeyboardOpenError::PermissionDenied)
    } else if found {
        Err(KeyboardOpenError::Other(
            last_error.unwrap_or_else(|| "unknown USB error".to_string()),
        ))
    } else {
        Err(KeyboardOpenError::NotFound)
    }
}

pub fn print_permission_report() -> Result<()> {
    let report = PermissionReport::collect();

    println!("Arch-Sense permission report");
    println!(
        "  Effective root: {}",
        if report.is_root { "yes" } else { "no" }
    );
    println!(
        "  Hardware group: {} ({})",
        HARDWARE_GROUP,
        if group_exists(HARDWARE_GROUP) {
            "exists"
        } else {
            "missing"
        }
    );
    println!("  USB keyboard: {}", usb_access_label(&report.usb));
    println!("  Sysfs write access:");

    for (path, access) in &report.sysfs {
        println!("    {}: {}", path.display(), path_access_label(access));
    }

    println!("  Config path: {}", config_path().display());

    if report.has_limited_access() {
        println!();
        println!("Fix: {}", setup_hint());
    }

    Ok(())
}

pub fn install_permissions() -> Result<()> {
    if !is_root() {
        return reexec_install_permissions();
    }

    install_permissions_as_root()
}

pub fn install_permissions_as_root() -> Result<()> {
    if !is_root() {
        bail!(
            "--install-permissions-root must run as root; use `arch-sense --install-permissions`"
        );
    }

    ensure_group()?;

    let target_user = invoking_user();
    let user_added = if let Some(user) = target_user.as_deref() {
        ensure_user_in_group(user)?
    } else {
        false
    };

    let binary = service_binary_path();
    write_root_file(Path::new(UDEV_RULE_PATH), &udev_rules(&binary))?;
    write_root_file(
        Path::new(PERMISSION_SERVICE_PATH),
        &permission_service(&binary),
    )?;

    apply_permissions_as_root()?;

    warn_command("udevadm", ["control", "--reload-rules"]);
    warn_command(
        "udevadm",
        [
            "trigger",
            "--subsystem-match=usb",
            "--attr-match=idVendor=04f2",
        ],
    );
    warn_command("udevadm", ["trigger", "--subsystem-match=platform"]);
    warn_command("systemctl", ["daemon-reload"]);
    warn_command(
        "systemctl",
        ["enable", "--now", "arch-sense-permissions.service"],
    );

    println!("arch-sense: installed rootless hardware permissions");
    println!("arch-sense: udev rules: {UDEV_RULE_PATH}");
    println!("arch-sense: permission service: {PERMISSION_SERVICE_PATH}");

    if let Some(user) = target_user {
        if user_added {
            println!(
                "arch-sense: added {user} to {HARDWARE_GROUP}; log out and back in before launching without sudo"
            );
        }
    } else {
        println!(
            "arch-sense: no invoking user detected; add your user with `sudo usermod -aG {HARDWARE_GROUP} <user>`"
        );
    }

    Ok(())
}

pub fn apply_permissions_as_root() -> Result<()> {
    if !is_root() {
        bail!("--apply-permissions must run as root; use `arch-sense --install-permissions`");
    }

    ensure_group()?;

    let existing_paths: Vec<PathBuf> = sysfs_write_paths()
        .into_iter()
        .filter(|path| path.exists())
        .collect();

    if !existing_paths.is_empty() {
        run_command(
            "chgrp",
            std::iter::once(OsString::from(HARDWARE_GROUP)).chain(
                existing_paths
                    .iter()
                    .map(|path| path.as_os_str().to_os_string()),
            ),
        )?;
        run_command(
            "chmod",
            std::iter::once(OsString::from("g+rw")).chain(
                existing_paths
                    .iter()
                    .map(|path| path.as_os_str().to_os_string()),
            ),
        )?;
    }

    let dir = config_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating config directory {}", dir.display()))?;
    run_command(
        "chgrp",
        [
            OsString::from(HARDWARE_GROUP),
            dir.as_os_str().to_os_string(),
        ],
    )?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o2775))
        .with_context(|| format!("setting permissions on {}", dir.display()))?;

    let config = config_path();
    if config.exists() {
        run_command(
            "chgrp",
            [
                OsString::from(HARDWARE_GROUP),
                config.as_os_str().to_os_string(),
            ],
        )?;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o664))
            .with_context(|| format!("setting permissions on {}", config.display()))?;
    }

    if existing_paths.is_empty() {
        eprintln!(
            "arch-sense: warning: no sysfs control files found; install/load linuwu_sense and rerun `arch-sense --apply-permissions` if controls remain locked"
        );
    }

    Ok(())
}

fn reexec_install_permissions() -> Result<()> {
    let exe = env::current_exe().context("resolving current executable for pkexec")?;
    let status = Command::new("pkexec")
        .arg(&exe)
        .arg(ROOT_INSTALL_FLAG)
        .status()
        .with_context(|| {
            format!(
                "starting pkexec; install polkit or run `sudo {} --install-permissions`",
                exe.display()
            )
        })?;

    if !status.success() {
        bail!("pkexec failed with status {status}; run `sudo arch-sense --install-permissions`");
    }

    Ok(())
}

fn sysfs_write_paths() -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(SYSFS_ATTRS.len() + 1);
    paths.push(PathBuf::from(PLATFORM_PROFILE));
    paths.extend(SYSFS_ATTRS.iter().map(|attr| PathBuf::from(ps(attr))));
    paths
}

fn path_write_access(path: &Path) -> PathAccess {
    match OpenOptions::new().write(true).open(path) {
        Ok(_) => PathAccess::Writable,
        Err(err) if err.kind() == ErrorKind::NotFound => PathAccess::Missing,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => PathAccess::PermissionDenied,
        Err(err) => PathAccess::Error(err.to_string()),
    }
}

fn effective_uid() -> Option<u32> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let uid_line = status.lines().find(|line| line.starts_with("Uid:"))?;
    uid_line.split_whitespace().nth(2)?.parse().ok()
}

fn is_root() -> bool {
    effective_uid() == Some(0)
}

fn group_exists(group: &str) -> bool {
    fs::read_to_string("/etc/group")
        .ok()
        .map(|groups| {
            groups.lines().any(|line| {
                line.split(':')
                    .next()
                    .map(|name| name == group)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn ensure_group() -> Result<()> {
    if group_exists(HARDWARE_GROUP) {
        return Ok(());
    }

    run_command("groupadd", ["--system", HARDWARE_GROUP])
}

fn invoking_user() -> Option<String> {
    env::var("SUDO_USER")
        .ok()
        .filter(|user| !user.is_empty() && user != "root")
        .or_else(|| {
            env::var("PKEXEC_UID")
                .ok()
                .and_then(|uid| user_from_uid(&uid))
        })
}

fn user_from_uid(uid: &str) -> Option<String> {
    fs::read_to_string("/etc/passwd")
        .ok()?
        .lines()
        .find_map(|line| {
            let mut parts = line.split(':');
            let name = parts.next()?;
            let _passwd = parts.next()?;
            let user_uid = parts.next()?;
            if user_uid == uid {
                Some(name.to_string())
            } else {
                None
            }
        })
        .filter(|user| !user.is_empty() && user != "root")
}

fn ensure_user_in_group(user: &str) -> Result<bool> {
    if user_in_group(user, HARDWARE_GROUP) {
        return Ok(false);
    }

    run_command("usermod", ["-aG", HARDWARE_GROUP, user])?;
    Ok(true)
}

fn user_in_group(user: &str, group: &str) -> bool {
    fs::read_to_string("/etc/group")
        .ok()
        .and_then(|groups| {
            groups.lines().find_map(|line| {
                let mut parts = line.split(':');
                let name = parts.next()?;
                if name != group {
                    return None;
                }

                let _passwd = parts.next()?;
                let _gid = parts.next()?;
                let members = parts.next().unwrap_or_default();
                Some(members.split(',').any(|member| member == user))
            })
        })
        .unwrap_or(false)
}

fn service_binary_path() -> PathBuf {
    let installed = PathBuf::from(INSTALLED_BINARY_PATH);
    if installed.exists() {
        installed
    } else {
        env::current_exe().unwrap_or(installed)
    }
}

fn udev_rules(binary: &Path) -> String {
    format!(
        r#"# Arch-Sense hardware permissions
# Managed by: arch-sense --install-permissions

# Let the active local user and the arch-sense group open the keyboard USB device.
ACTION=="add|change", SUBSYSTEM=="usb", ENV{{DEVTYPE}}=="usb_device", ATTR{{idVendor}}=="04f2", ATTR{{idProduct}}=="0117", TAG+="uaccess", GROUP="{HARDWARE_GROUP}", MODE="0660"

# Reapply sysfs permissions whenever the Acer platform device is announced.
ACTION=="add|change", SUBSYSTEM=="platform", KERNEL=="acer-wmi", RUN+="{binary} --apply-permissions"
"#,
        binary = binary.display()
    )
}

fn permission_service(binary: &Path) -> String {
    format!(
        r#"[Unit]
Description=Prepare Arch-Sense hardware permissions
After=systemd-modules-load.service systemd-udevd.service
Wants=systemd-udevd.service

[Service]
Type=oneshot
ExecStartPre=-/usr/bin/modprobe linuwu_sense
ExecStart={binary} --apply-permissions

[Install]
WantedBy=multi-user.target
"#,
        binary = binary.display()
    )
}

fn write_root_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    if fs::read_to_string(path).ok().as_deref() != Some(content) {
        fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    }

    fs::set_permissions(path, fs::Permissions::from_mode(0o644))
        .with_context(|| format!("setting permissions on {}", path.display()))?;
    Ok(())
}

fn run_command<I, S>(program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let output = Command::new(program)
        .args(&args)
        .output()
        .with_context(|| format!("starting {program}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        bail!("{program} failed with status {}", output.status);
    }

    bail!("{program} failed with status {}: {stderr}", output.status)
}

fn warn_command<I, S>(program: &str, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    if let Err(err) = run_command(program, args) {
        eprintln!("arch-sense: warning: {err}");
    }
}

fn usb_access_label(access: &UsbAccess) -> String {
    match access {
        UsbAccess::Accessible => "accessible".to_string(),
        UsbAccess::PermissionDenied => format!("permission denied; {}", setup_hint()),
        UsbAccess::NotFound => "not found".to_string(),
        UsbAccess::Error(err) => format!("error: {err}"),
    }
}

fn path_access_label(access: &PathAccess) -> String {
    match access {
        PathAccess::Writable => "writable".to_string(),
        PathAccess::Missing => "missing".to_string(),
        PathAccess::PermissionDenied => format!("permission denied; {}", setup_hint()),
        PathAccess::Error(err) => format!("error: {err}"),
    }
}
