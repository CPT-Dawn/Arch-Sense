mod common;

use std::fs;

use arch_sense::fixture::FakeMachine;
use arch_sense::keyboard::RecordingTransport;
use crossterm::event::KeyCode;

use common::Harness;

#[test]
fn choosing_a_thermal_profile_writes_it_to_the_platform_profile() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_until("controls to load", |app| {
        app.line_with("Thermal Profile")
            .is_some_and(|line| line.contains("Balanced"))
    });

    app.press(KeyCode::Right);
    app.press(KeyCode::Enter);

    let profile = fake.machine().platform_profile();
    app.wait_until("the profile write", |_| {
        fs::read_to_string(&profile).unwrap() == "performance"
    });
    app.wait_until("the row to show the applied profile", |app| {
        app.line_with("Thermal Profile").is_some_and(|line| {
            line.contains("Performance") && !line.contains("PREVIEW") && !line.contains("APPLY")
        })
    });
}
