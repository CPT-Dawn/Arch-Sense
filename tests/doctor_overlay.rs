//! The Doctor overlay: `d` shows the same checks `arch-sense doctor` prints.

mod common;

use arch_sense::doctor::{self, Membership};
use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::RecordingTransport;
use crossterm::event::KeyCode;

use common::Harness;

#[test]
fn d_opens_the_doctor_with_the_same_checks_as_the_cli_and_esc_closes_it() {
    let fake = FakeMachine::builder()
        .healthy_ph16_71()
        .login(&["dawn"], &[998, 1000])
        .acer_wmi_loaded()
        .build()
        .expect("fake machine");
    let machine = fake.machine();
    let report = doctor::diagnose(&machine, Membership::current(&machine));
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();
    assert!(!app.screen().contains("Doctor"));

    app.press(KeyCode::Char('d'));
    let screen = app.screen();

    assert!(screen.contains("Doctor"), "{screen}");
    for check in report.checks() {
        let line = format!("{} {}", check.status.mark(), check.name);
        assert!(screen.contains(&line), "{line:?} missing from\n{screen}");
        assert!(
            screen.contains(&check.detail),
            "{:?} missing from\n{screen}",
            check.detail
        );
    }
    assert!(screen.contains("Log out and back in"), "{screen}");

    app.press(KeyCode::Esc);
    assert!(!app.screen().contains("Log out and back in"));
    assert!(app.is_running());
}

#[test]
fn the_doctor_overlay_fits_a_small_terminal_and_scrolls() {
    let fake = FakeMachine::builder()
        .module_missing()
        .build()
        .expect("fake machine");
    let mut app = Harness::start_sized(&fake, RecordingTransport::default(), 80, 24);
    app.wait_for_scan();

    app.press(KeyCode::Char('d'));
    let first = app.screen();
    assert!(first.contains("✗ Kernel module"), "{first}");
    assert!(
        !first.contains("v1 leftovers"),
        "everything fits in 24 lines?\n{first}"
    );

    app.press_n(KeyCode::Down, 40);
    let scrolled = app.screen();
    assert!(scrolled.contains("v1 leftovers"), "{scrolled}");
}
