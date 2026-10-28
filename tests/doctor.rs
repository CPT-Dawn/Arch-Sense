//! The Doctor: what it says about a fake machine, and that it always says how to fix it.

use arch_sense::doctor::{self, Check, Membership, Report, Status};
use arch_sense::fixture::{FakeMachine, FakeMachineBuilder};

fn diagnose(builder: FakeMachineBuilder, membership: Membership) -> Report {
    let fake = builder.build().expect("fake machine");
    doctor::diagnose(&fake.machine(), membership)
}

fn healthy() -> FakeMachineBuilder {
    FakeMachine::builder().healthy_ph16_71()
}

fn check<'a>(report: &'a Report, name: &str) -> &'a Check {
    report
        .checks()
        .iter()
        .find(|check| check.name == name)
        .unwrap_or_else(|| panic!("no {name:?} check in:\n{report}"))
}

#[test]
fn a_healthy_ph16_71_passes_every_check() {
    let report = diagnose(healthy(), Membership::Member);

    for check in report.checks() {
        assert_eq!(check.status, Status::Ok, "{report}");
    }
    assert!(!report.has_errors());
    assert_eq!(check(&report, "Laptop").detail, "Acer Predator PH16-71");
}

/// The one check that isn't Ok, asserting every other check is.
fn only_problem(report: &Report) -> &Check {
    let problems: Vec<&Check> = report
        .checks()
        .iter()
        .filter(|check| check.status != Status::Ok)
        .collect();
    assert_eq!(problems.len(), 1, "expected one problem in:\n{report}");
    problems[0]
}

fn remedy(check: &Check) -> &str {
    check.remedy.as_deref().expect("a remedy")
}

#[test]
fn a_missing_module_is_an_error_that_points_at_dkms() {
    let report = diagnose(
        FakeMachine::builder()
            .module_missing()
            .acer("Predator PH16-71")
            .gcc_kernel()
            .dkms_registered()
            .secure_boot(false)
            .usb_hid_keyboard()
            .boot_restore(),
        Membership::Member,
    );

    assert!(report.has_errors());
    assert_eq!(report.exit_code(), 1);
    assert_eq!(check(&report, "Kernel module").status, Status::Error);
    let dkms = check(&report, "DKMS");
    assert_eq!(dkms.status, Status::Error);
    assert!(dkms.detail.contains("not built"), "{report}");
    assert!(remedy(dkms).contains("dkms autoinstall"), "{report}");
    assert_eq!(check(&report, "Sense Interface").status, Status::Error);
}

#[test]
fn a_built_but_unloaded_module_on_secure_boot_explains_signing() {
    let report = diagnose(
        FakeMachine::builder()
            .module_missing()
            .acer("Predator PH16-71")
            .gcc_kernel()
            .dkms_built()
            .secure_boot(true),
        Membership::Member,
    );

    let secure_boot = check(&report, "Secure Boot");
    assert_eq!(secure_boot.status, Status::Warn);
    assert!(remedy(secure_boot).contains("mokutil"));
    // The commands run on the real machine, never on the fake tree under test.
    assert!(
        remedy(secure_boot).contains("`sudo mokutil --import /var/lib/dkms/mok.pub`"),
        "{}",
        remedy(secure_boot)
    );
    assert!(remedy(secure_boot).contains("/etc/dkms/framework.conf"));
    assert!(remedy(check(&report, "Kernel module")).contains("modprobe linuwu_sense"));
    assert_eq!(check(&report, "DKMS").status, Status::Ok);
}

#[test]
fn nothing_installed_says_which_package_to_install() {
    let report = diagnose(
        FakeMachine::builder()
            .module_missing()
            .acer("Nitro AN515-58")
            .gcc_kernel(),
        Membership::Member,
    );

    let dkms = check(&report, "DKMS");
    assert_eq!(dkms.status, Status::Error);
    assert!(remedy(dkms).contains("paru -S linuwu-sense-dkms-git"));
}

#[test]
fn acer_wmi_loaded_is_an_error() {
    let report = diagnose(healthy().acer_wmi_loaded(), Membership::Member);

    let problem = only_problem(&report);
    assert_eq!(problem.name, "acer_wmi");
    assert_eq!(problem.status, Status::Error);
    assert!(remedy(problem).contains("modprobe -r acer_wmi"));
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn a_clang_kernel_without_the_module_says_to_build_with_llvm() {
    let report = diagnose(
        FakeMachine::builder()
            .module_missing()
            .acer("Predator PHN16-71")
            .clang_kernel()
            .dkms_registered(),
        Membership::Member,
    );

    let compiler = check(&report, "Kernel compiler");
    assert_eq!(compiler.status, Status::Warn);
    assert!(compiler.detail.contains("clang"));
    assert!(remedy(compiler).contains("clang lld llvm"));
}

#[test]
fn a_clang_kernel_with_the_module_loaded_is_fine() {
    let report = diagnose(healthy().clang_kernel(), Membership::Member);

    let compiler = check(&report, "Kernel compiler");
    assert_eq!(compiler.status, Status::Ok);
    assert!(compiler.detail.contains("clang"));
}

#[test]
fn a_user_outside_the_group_is_told_to_join_it() {
    let report = diagnose(healthy(), Membership::NotMember);

    let problem = only_problem(&report);
    assert_eq!(problem.name, "Permissions");
    assert_eq!(problem.status, Status::Error);
    assert!(remedy(problem).contains("usermod -aG arch-sense"));
}

#[test]
fn a_stale_login_is_told_to_log_in_again() {
    let report = diagnose(healthy(), Membership::StaleLogin);

    let problem = only_problem(&report);
    assert_eq!(problem.name, "Permissions");
    assert_eq!(problem.status, Status::Error);
    assert!(remedy(problem).contains("Log out and back in"));
}

#[test]
fn an_unsupported_keyboard_is_a_warning_with_a_way_to_help() {
    let report = diagnose(
        healthy()
            .without(|m| m.usb_devices_dir())
            .jing_mold_keyboard()
            .acer("Predator PH16-72"),
        Membership::Member,
    );

    let problem = only_problem(&report);
    assert_eq!(problem.name, "Lighting Backend");
    assert_eq!(problem.status, Status::Warn);
    assert!(problem.detail.contains("05af:666a"), "{report}");
    assert!(remedy(problem).contains("issues"));
    assert!(remedy(problem).contains("Arch-Sense#contributing"));
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn a_four_zone_keyboard_is_detected() {
    let report = diagnose(
        healthy().without(|m| m.usb_devices_dir()).four_zoned_kb(),
        Membership::Member,
    );

    let lighting = check(&report, "Lighting Backend");
    assert_eq!(lighting.status, Status::Ok);
    assert_eq!(lighting.detail, "Four-Zone");
}

#[test]
fn the_doctor_agrees_with_the_app_that_four_zone_beats_usb_hid() {
    let report = diagnose(healthy().four_zoned_kb(), Membership::Member);

    assert_eq!(check(&report, "Lighting Backend").detail, "Four-Zone");
}

#[test]
fn an_ene_kb5130_model_warns_about_the_known_limitation() {
    let report = diagnose(
        healthy()
            .without(|m| m.usb_devices_dir())
            .four_zoned_kb()
            .ene_kb5130()
            .acer("Predator PHN16S-71"),
        Membership::Member,
    );

    let problem = only_problem(&report);
    assert_eq!(problem.name, "Lighting Backend");
    assert_eq!(problem.status, Status::Warn);
    assert!(problem.detail.contains("ENE KB5130"));
    assert!(remedy(problem).contains("known limitation"));
}

#[test]
fn boot_restore_off_is_only_a_note() {
    let report = diagnose(
        healthy().without(|m| m.boot_restore_link()),
        Membership::Member,
    );

    let problem = only_problem(&report);
    assert_eq!(problem.name, "Boot Restore");
    assert_eq!(problem.status, Status::Info);
    assert!(remedy(problem).contains("systemctl enable arch-sense.service"));
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn v1_leftovers_are_listed_for_removal() {
    let report = diagnose(healthy().v1_leftovers(), Membership::Member);

    let problem = only_problem(&report);
    assert_eq!(problem.name, "v1 leftovers");
    assert!(remedy(problem).contains("systemctl disable arch-sense-permissions.service"));
    assert!(remedy(problem).contains(
        "sudo rm /etc/udev/rules.d/70-arch-sense.rules /etc/systemd/system/arch-sense-permissions.service"
    ));
}

#[test]
fn a_model_linuwu_sense_does_not_know_is_explained() {
    let report = diagnose(
        healthy()
            .acer("Predator PH18-72")
            .without(|m| m.predator_sense_dir()),
        Membership::Member,
    );

    let sense = check(&report, "Sense Interface");
    assert_eq!(sense.status, Status::Error);
    assert!(remedy(sense).contains("predator_v4=1"));
    assert!(!remedy(sense).contains("PH16-71"), "{report}");
}

#[test]
fn a_ph16_71_without_its_dmi_entry_gets_the_known_regression_fix() {
    let report = diagnose(
        FakeMachine::builder().ph16_71_without_dmi_entry(),
        Membership::Member,
    );

    let sense = only_problem(&report);
    assert_eq!(sense.name, "Sense Interface");
    assert_eq!(sense.status, Status::Error);
    let remedy = remedy(sense);
    assert!(remedy.contains("known upstream regression"), "{remedy}");
    assert!(
        remedy.contains("https://github.com/0x7375646F/Linuwu-Sense/pull/152"),
        "{remedy}"
    );
    assert!(remedy.contains("paru -S linuwu-sense-dkms-git"), "{remedy}");
    // The commands run on the real machine, never on the fake tree under test.
    assert!(
        remedy.contains(
            "`echo 'options linuwu_sense predator_v4=1' | sudo tee /etc/modprobe.d/linuwu-sense-ph16-71.conf`"
        ),
        "{remedy}"
    );
    assert!(
        remedy.contains("`sudo modprobe -r linuwu_sense && sudo modprobe linuwu_sense`"),
        "{remedy}"
    );
}

#[test]
fn every_check_that_is_not_ok_carries_a_remedy() {
    let machines = || {
        [
            FakeMachine::builder().module_missing(),
            FakeMachine::builder()
                .clang_kernel()
                .dkms_registered()
                .secure_boot(true),
            healthy()
                .acer_wmi_loaded()
                .v1_leftovers()
                .without(|m| m.boot_restore_link()),
            healthy().ene_kb5130(),
            healthy()
                .without(|m| m.usb_devices_dir())
                .jing_mold_keyboard(),
            FakeMachine::builder().predator_sense(),
            FakeMachine::builder().ph16_71_without_dmi_entry(),
        ]
    };
    for membership in [
        Membership::Member,
        Membership::NoGroup,
        Membership::NotMember,
        Membership::StaleLogin,
    ] {
        for builder in machines() {
            let report = diagnose(builder, membership);
            for check in report.checks() {
                assert_eq!(
                    check.status == Status::Ok,
                    check.remedy.is_none(),
                    "{report}"
                );
            }
        }
    }
}

fn membership_of(builder: FakeMachineBuilder) -> Membership {
    let fake = builder.build().expect("fake machine");
    Membership::current(&fake.machine())
}

#[test]
fn membership_comes_from_the_session_and_the_group_file() {
    let user = || FakeMachine::builder();
    assert_eq!(
        membership_of(user().login(&["dawn"], &[966, 998, 1000])),
        Membership::Member
    );
    assert_eq!(
        membership_of(user().login(&["dawn"], &[998, 1000])),
        Membership::StaleLogin
    );
    assert_eq!(
        membership_of(user().login(&["someone"], &[998, 1000])),
        Membership::NotMember
    );
    assert_eq!(membership_of(user()), Membership::NoGroup);
}

#[test]
fn the_report_prints_one_line_per_check_with_remedies_underneath() {
    let text = diagnose(healthy(), Membership::NotMember).to_string();

    assert!(text.contains("✓ Laptop"), "{text}");
    assert!(text.contains("✗ Permissions"), "{text}");
    assert!(
        text.contains("→ `sudo usermod -aG arch-sense $USER`"),
        "{text}"
    );
}
