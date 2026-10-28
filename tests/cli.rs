use arch_sense::cli::{Cli, Command};
use arch_sense::commands;
use arch_sense::doctor::Membership;
use arch_sense::fixture::{FakeMachine, FakeMachineBuilder};
use clap::Parser;

#[test]
fn the_pkexec_self_installer_flags_are_gone() {
    for flag in [
        "--install-permissions",
        "--install-permissions-root",
        "--apply-permissions",
    ] {
        assert!(
            Cli::try_parse_from(["arch-sense", flag]).is_err(),
            "{flag} should no longer be accepted"
        );
    }
}

#[test]
fn help_describes_what_arch_sense_actually_drives() {
    use clap::CommandFactory;
    let help = Cli::command().render_long_help().to_string();

    for words in [
        "Acer Predator and Nitro",
        "Linuwu-Sense",
        "Fans (Auto or Max)",
        "Boot Restore",
        "Saved Lighting",
    ] {
        assert!(help.contains(words), "{words:?} missing from\n{help}");
    }
    assert!(!help.contains("fan speeds"), "{help}");
    assert!(!help.contains("RGB settings"), "{help}");
}

#[test]
fn boot_restore_keeps_its_flag() {
    assert!(Cli::try_parse_from(["arch-sense", "--apply"]).is_ok());
}

#[test]
fn demo_is_a_flag_and_excludes_boot_restore() {
    assert!(Cli::try_parse_from(["arch-sense", "--demo"]).is_ok());
    assert!(Cli::try_parse_from(["arch-sense", "--demo", "--apply"]).is_err());
}

#[test]
fn doctor_is_a_subcommand() {
    let cli = Cli::try_parse_from(["arch-sense", "doctor"]).expect("doctor parses");
    assert_eq!(cli.command, Some(Command::Doctor));
}

#[test]
fn doctor_prints_the_checklist_and_exits_non_zero_only_on_errors() {
    let run = |builder: FakeMachineBuilder, membership| {
        let fake = builder.build().expect("fake machine");
        let mut out = Vec::new();
        let code = commands::doctor(&fake.machine(), membership, &mut out).expect("doctor runs");
        (code, String::from_utf8(out).expect("utf-8"))
    };

    let (code, text) = run(FakeMachine::builder().healthy_ph16_71(), Membership::Member);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("✓ Kernel module"), "{text}");

    let (code, _) = run(
        FakeMachine::builder()
            .healthy_ph16_71()
            .without(|m| m.boot_restore_link())
            .v1_leftovers(),
        Membership::Member,
    );
    assert_eq!(code, 0, "warnings and notes alone don't fail");

    let (code, text) = run(
        FakeMachine::builder().healthy_ph16_71(),
        Membership::NotMember,
    );
    assert_eq!(code, 1);
    assert!(text.contains("✗ Permissions"), "{text}");
}
