use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::config::config_path;
use crate::keyboard::{UsbAccess, keyboard_access};
use crate::machine::MachineRoot;

pub(crate) const HARDWARE_GROUP: &str = "arch-sense";

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
    pub(crate) fn collect(m: &MachineRoot) -> Self {
        Self {
            is_root: is_root(m),
            sysfs: sysfs_write_paths(m)
                .into_iter()
                .map(|path| {
                    let access = path_write_access(&path);
                    (path, access)
                })
                .collect(),
            usb: keyboard_access(m),
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
}

pub(crate) fn setup_hint() -> &'static str {
    "run `arch-sense --install-permissions` once, then log out and back in if prompted"
}

pub fn print_permission_report(m: &MachineRoot) -> Result<()> {
    let report = PermissionReport::collect(m);

    println!("Arch-Sense permission report");
    println!(
        "  Effective root: {}",
        if report.is_root { "yes" } else { "no" }
    );
    println!(
        "  Hardware group: {} ({})",
        HARDWARE_GROUP,
        if group_exists(m, HARDWARE_GROUP) {
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

    println!("  Config path: {}", config_path(m).display());

    if report.has_limited_access() {
        println!();
        println!("Fix: {}", setup_hint());
    }

    Ok(())
}

pub fn install_permissions(m: &MachineRoot) -> Result<()> {
    if !is_root(m) {
        return reexec_install_permissions();
    }

    install_permissions_as_root(m)
}

pub fn install_permissions_as_root(m: &MachineRoot) -> Result<()> {
    if !is_root(m) {
        bail!(
            "--install-permissions-root must run as root; use `arch-sense --install-permissions`"
        );
    }

    ensure_group(m)?;

    let target_user = invoking_user(m);
    let user_added = if let Some(user) = target_user.as_deref() {
        ensure_user_in_group(m, user)?
    } else {
        false
    };

    let binary = service_binary_path(m);
    let udev_rule = m.udev_rule();
    let permission_service_path = m.permission_service();
    write_root_file(&udev_rule, &udev_rules(&binary))?;
    write_root_file(&permission_service_path, &permission_service(&binary))?;

    apply_permissions_as_root(m)?;

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
    println!("arch-sense: udev rules: {}", udev_rule.display());
    println!(
        "arch-sense: permission service: {}",
        permission_service_path.display()
    );

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

pub fn apply_permissions_as_root(m: &MachineRoot) -> Result<()> {
    if !is_root(m) {
        bail!("--apply-permissions must run as root; use `arch-sense --install-permissions`");
    }

    ensure_group(m)?;

    let existing_paths: Vec<PathBuf> = sysfs_write_paths(m)
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

    let dir = m.state_dir();
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

    let config = config_path(m);
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
    let exe = env::current_exe().context("resolving current executable for elevation")?;

    // Try pkexec first for a GUI-friendly experience
    match Command::new("pkexec")
        .arg(&exe)
        .arg(ROOT_INSTALL_FLAG)
        .status()
    {
        Ok(status) if status.success() => return Ok(()),
        _ => {
            // If pkexec fails (cancelled or not found), try sudo for terminal robustness
            eprintln!("arch-sense: pkexec failed or cancelled; falling back to sudo...");
            let status = Command::new("sudo")
                .arg(&exe)
                .arg(ROOT_INSTALL_FLAG)
                .status()
                .context("failed to execute sudo")?;

            if !status.success() {
                bail!(
                    "elevation failed via both pkexec and sudo; manually run `sudo arch-sense --install-permissions`"
                )
            }
        }
    }

    Ok(())
}

fn sysfs_write_paths(m: &MachineRoot) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(SYSFS_ATTRS.len() + 1);
    paths.push(m.platform_profile());
    paths.extend(SYSFS_ATTRS.iter().map(|attr| m.predator_sense_attr(attr)));
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

fn effective_uid(m: &MachineRoot) -> Option<u32> {
    let status = fs::read_to_string(m.proc_self_status()).ok()?;
    let uid_line = status.lines().find(|line| line.starts_with("Uid:"))?;
    uid_line.split_whitespace().nth(2)?.parse().ok()
}

fn is_root(m: &MachineRoot) -> bool {
    effective_uid(m) == Some(0)
}

fn group_exists(m: &MachineRoot, group: &str) -> bool {
    fs::read_to_string(m.etc_group())
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

fn ensure_group(m: &MachineRoot) -> Result<()> {
    if group_exists(m, HARDWARE_GROUP) {
        return Ok(());
    }

    run_command("groupadd", ["--system", HARDWARE_GROUP])
}

fn invoking_user(m: &MachineRoot) -> Option<String> {
    env::var("SUDO_USER")
        .ok()
        .filter(|user| !user.is_empty() && user != "root")
        .or_else(|| {
            env::var("PKEXEC_UID")
                .ok()
                .and_then(|uid| user_from_uid(m, &uid))
        })
}

fn user_from_uid(m: &MachineRoot, uid: &str) -> Option<String> {
    fs::read_to_string(m.etc_passwd())
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

fn ensure_user_in_group(m: &MachineRoot, user: &str) -> Result<bool> {
    if user_in_group(m, user, HARDWARE_GROUP) {
        return Ok(false);
    }

    run_command("usermod", ["-aG", HARDWARE_GROUP, user])?;
    Ok(true)
}

fn user_in_group(m: &MachineRoot, user: &str, group: &str) -> bool {
    fs::read_to_string(m.etc_group())
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

fn service_binary_path(m: &MachineRoot) -> PathBuf {
    let installed = m.installed_binary();
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
