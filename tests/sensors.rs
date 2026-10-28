//! The gauge cards and Turbo indicator: what the dashboard shows from sensors.

mod common;

use arch_sense::fixture::{FakeMachine, FakeMachineBuilder};
use arch_sense::keyboard::RecordingTransport;

use common::Harness;

fn screen_of(laptop: FakeMachineBuilder) -> String {
    let fake = laptop.build().expect("fake machine");
    let mut app = Harness::start(&fake, RecordingTransport::default());
    app.wait_for_scan();
    app.screen()
}

fn cards(laptop: FakeMachineBuilder) -> String {
    let screen = screen_of(laptop);
    screen
        .lines()
        .skip(1)
        .take(5)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_amd_laptop_shows_tctl_and_the_amdgpu_edge_temperature() {
    let cards = cards(
        FakeMachine::builder()
            .predator_sense()
            .amd_cpu(61)
            .amd_gpu(48),
    );

    assert!(cards.contains("61°C"), "{cards}");
    assert!(cards.contains("48°C"), "{cards}");
}

#[test]
fn an_intel_laptop_shows_the_package_temperature_not_the_nvme_drive() {
    let cards = cards(
        FakeMachine::builder()
            .predator_sense()
            .intel_cpu(55)
            .amd_gpu(44),
    );

    assert!(cards.contains("55°C"), "{cards}");
    assert!(!cards.contains("33°C"), "{cards}");
}

#[test]
fn an_nvidia_hwmon_sensor_gives_the_gpu_temperature() {
    let cards = cards(
        FakeMachine::builder()
            .predator_sense()
            .intel_cpu(55)
            .nvidia_hwmon_gpu(44),
    );

    assert!(cards.contains("44°C"), "{cards}");
}

#[test]
fn linuwus_unlabelled_acer_chip_maps_temp1_to_cpu_and_temp2_to_gpu_and_fans_in_order() {
    let cards = cards(
        FakeMachine::builder()
            .predator_sense()
            .acer_sensors(66, 59, 3100, 3300),
    );

    assert!(cards.contains("66°C"), "{cards}");
    assert!(cards.contains("59°C"), "{cards}");
    assert!(cards.contains("3100 / 3300 rpm"), "{cards}");
}

#[test]
fn nvidia_smi_fills_in_when_no_hwmon_sensor_belongs_to_the_gpu() {
    let cards = cards(
        FakeMachine::builder()
            .predator_sense()
            .intel_cpu(55)
            .nvidia_smi(71),
    );

    assert!(cards.contains("71°C"), "{cards}");
}

#[test]
fn hwmon_wins_over_nvidia_smi() {
    let cards = cards(
        FakeMachine::builder()
            .predator_sense()
            .intel_cpu(55)
            .amd_gpu(48)
            .nvidia_smi(90),
    );

    assert!(cards.contains("48°C"), "{cards}");
    assert!(!cards.contains("90°C"), "{cards}");
}

#[test]
fn a_gpu_with_no_sensor_at_all_says_unavailable() {
    let cards = cards(FakeMachine::builder().predator_sense().intel_cpu(55));

    assert!(cards.contains("unavailable"), "{cards}");
}

#[test]
fn the_battery_card_shows_charge_state_watts_and_the_charge_limit() {
    let cards = cards(
        FakeMachine::builder()
            .predator_sense()
            .battery(78, "Charging", 24_100_000),
    );

    for part in ["78%", "Charging", "24.1 W", "limit 80%"] {
        assert!(cards.contains(part), "{part:?} missing from\n{cards}");
    }
}

#[test]
fn a_discharging_battery_reads_as_on_battery() {
    let cards = cards(FakeMachine::builder().predator_sense().battery(
        41,
        "Discharging",
        9_800_000,
    ));

    assert!(cards.contains("41%"), "{cards}");
    assert!(cards.contains("On battery"), "{cards}");
    assert!(cards.contains("9.8 W"), "{cards}");
}

#[test]
fn a_laptop_without_a_battery_says_so() {
    let cards = cards(FakeMachine::builder().predator_sense());

    assert!(cards.contains("No battery"), "{cards}");
}

#[test]
fn the_turbo_indicator_shows_only_when_the_laptop_has_a_turbo_key() {
    let engaged = screen_of(FakeMachine::builder().predator_sense().turbo(true));
    let off = screen_of(FakeMachine::builder().predator_sense().turbo(false));
    let none = screen_of(FakeMachine::builder().predator_sense());

    let header = |screen: &str| screen.lines().next().unwrap_or_default().to_owned();
    assert!(header(&engaged).contains("TURBO"), "{}", header(&engaged));
    assert!(header(&off).contains("Turbo off"), "{}", header(&off));
    assert!(
        !header(&none).to_lowercase().contains("turbo"),
        "{}",
        header(&none)
    );
}
