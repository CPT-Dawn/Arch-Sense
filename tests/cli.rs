use arch_sense::cli::Cli;
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
fn boot_restore_keeps_its_flag() {
    assert!(Cli::try_parse_from(["arch-sense", "--apply"]).is_ok());
}
