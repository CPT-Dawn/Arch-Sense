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
    app.wait_until("the Doctor's checks", |app| {
        app.screen().contains("Log out and back in")
    });
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
    app.wait_until("the Doctor's checks", |app| {
        app.screen().contains("✗ Kernel module")
    });
    let first = app.screen();
    assert!(
        !first.contains("v1 leftovers"),
        "everything fits in 24 lines?\n{first}"
    );

    app.press_n(KeyCode::Down, 40);
    let scrolled = app.screen();
    assert!(scrolled.contains("v1 leftovers"), "{scrolled}");
}

#[test]
fn the_doctor_runs_off_the_ui_thread_and_says_so_until_it_is_done() {
    let fake = FakeMachine::builder()
        .healthy_ph16_71()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();

    app.press(KeyCode::Char('d'));
    let pending = app.screen_untouched();
    assert!(pending.contains("Running checks…"), "{pending}");
    assert!(!pending.contains("✓ Kernel module"), "{pending}");

    app.wait_until("the Doctor's checks", |app| {
        app.screen().contains("✓ Kernel module")
    });
    assert!(!app.screen().contains("Running checks…"));
}

#[test]
fn a_doctor_result_that_lands_after_the_overlay_closed_stays_closed() {
    let fake = FakeMachine::builder()
        .healthy_ph16_71()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();

    app.press(KeyCode::Char('d'));
    app.press(KeyCode::Esc);
    // Give the worker ample time to finish the diagnosis it was asked for.
    for _ in 0..20 {
        assert!(!app.screen().contains("Doctor"), "{}", app.screen());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(app.is_running());
}
