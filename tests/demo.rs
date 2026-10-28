//! Demo mode: a simulated Predator laptop that lives and dies in a temp directory.

mod common;

use std::path::{Path, PathBuf};

use arch_sense::demo::Demo;
use arch_sense::keyboard::RecordingTransport;
use crossterm::event::KeyCode;

use common::Harness;

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = vec![];
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk(&path));
        }
        found.push(path);
    }
    found
}

/// Every file under `root` with its contents, excluding the drifting sensor tree.
fn controls(root: &Path) -> Vec<(PathBuf, String)> {
    let mut files: Vec<_> = walk(root)
        .into_iter()
        .filter(|p| p.is_file() && !p.starts_with(root.join("sys/class/hwmon")))
        .filter(|p| !p.starts_with(root.join("sys/class/power_supply")))
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            (p, text)
        })
        .collect();
    files.sort();
    files
}

#[test]
fn the_demo_laptop_is_a_named_predator_with_everything_detected() {
    let demo = Demo::start().expect("demo");
    let mut app = Harness::start(demo.fake(), RecordingTransport::default());
    app.wait_for_scan();
    let screen = app.screen();

    assert!(screen.contains("Predator PH16-71"), "{screen}");
    assert!(!screen.contains("unavailable"), "{screen}");
    assert!(screen.contains("Wave"), "{screen}");
}

#[test]
fn drifting_changes_what_the_sensors_report() {
    let mut demo = Demo::start().expect("demo");
    let before = demo.readings();
    for _ in 0..12 {
        demo.drift();
    }
    assert_ne!(demo.readings(), before);

    let mut app = Harness::start(demo.fake(), RecordingTransport::default());
    app.wait_for_scan();
    assert!(app.screen().contains("°C"));
}

#[test]
fn the_sensor_files_on_disk_really_move() {
    let mut demo = Demo::start().expect("demo");
    let cpu = demo.machine().hwmon_dir().join("hwmon0/temp1_input");
    let seen: std::collections::BTreeSet<_> = (0..20)
        .map(|_| {
            demo.drift();
            std::fs::read_to_string(&cpu).expect("readable mid-drift")
        })
        .collect();
    assert!(seen.len() > 5, "{seen:?}");
}

#[test]
fn a_changed_control_lands_in_the_demo_tree() {
    let demo = Demo::start().expect("demo");
    let root = demo.fake().path().to_path_buf();
    let mut app = Harness::start(demo.fake(), RecordingTransport::default());
    app.wait_for_scan();
    let before = controls(&root);
    for _ in 0..3 {
        app.press(KeyCode::Down);
        app.press(KeyCode::Right);
    }
    app.wait_until("a write", |_| controls(&root) != before);
}

#[test]
fn everything_stays_under_the_root_and_dies_with_the_demo() {
    let demo = Demo::start().expect("demo");
    let root = demo.fake().path().to_path_buf();
    assert!(walk(&root).iter().all(|p| p.starts_with(&root)));
    drop(demo);
    assert!(!root.exists());
}
