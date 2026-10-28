//! Capability detection: which Sense Interface a laptop has and which Capabilities work.

use arch_sense::capabilities::{Capability, SenseInterface, Turbo, detect};
use arch_sense::fixture::FakeMachine;
use arch_sense::machine::MachineRoot;

#[test]
fn a_full_predator_has_every_capability() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");

    let laptop = detect(&fake.machine());

    assert_eq!(laptop.sense_interface, SenseInterface::Predator);
    assert_eq!(
        laptop.capabilities(),
        vec![
            Capability::ThermalProfile,
            Capability::Fans,
            Capability::ChargeLimit,
            Capability::BatteryCalibration,
            Capability::UsbCharging,
            Capability::BootAnimation,
            Capability::LcdOverdrive,
            Capability::BacklightTimeout,
        ]
    );
}

#[test]
fn a_nitro_has_the_nitro_sense_controls() {
    let fake = FakeMachine::builder()
        .nitro_sense()
        .build()
        .expect("fake machine");

    let laptop = detect(&fake.machine());

    assert_eq!(laptop.sense_interface, SenseInterface::Nitro);
    assert_eq!(
        laptop.capabilities(),
        vec![
            Capability::ThermalProfile,
            Capability::Fans,
            Capability::ChargeLimit,
            Capability::BatteryCalibration,
            Capability::UsbCharging,
            Capability::BacklightTimeout,
        ]
    );
}

#[test]
fn a_nitro_v4_has_the_full_set_like_a_predator() {
    let fake = FakeMachine::builder()
        .nitro_v4()
        .build()
        .expect("fake machine");

    let laptop = detect(&fake.machine());

    assert_eq!(laptop.sense_interface, SenseInterface::Nitro);
    assert_eq!(
        laptop.capabilities(),
        vec![
            Capability::ThermalProfile,
            Capability::Fans,
            Capability::ChargeLimit,
            Capability::BatteryCalibration,
            Capability::UsbCharging,
            Capability::BootAnimation,
            Capability::LcdOverdrive,
            Capability::BacklightTimeout,
        ]
    );
}

#[test]
fn without_the_module_there_is_no_sense_interface_and_nothing_to_control() {
    let fake = FakeMachine::builder()
        .module_missing()
        .build()
        .expect("fake machine");

    let laptop = detect(&fake.machine());

    assert_eq!(laptop.sense_interface, SenseInterface::None);
    assert!(laptop.capabilities().is_empty());
}

#[test]
fn thermal_profile_choices_are_exactly_what_the_firmware_offers() {
    let fake = FakeMachine::builder()
        .nitro_v4()
        .build()
        .expect("fake machine");

    let laptop = detect(&fake.machine());

    assert_eq!(
        laptop.thermal_profiles,
        [
            "low-power",
            "quiet",
            "balanced",
            "balanced-performance",
            "performance"
        ]
    );
}

#[test]
fn a_laptop_without_a_platform_profile_has_no_thermal_profile() {
    let fake = FakeMachine::builder()
        .nitro_sense()
        .without(MachineRoot::platform_profile)
        .without(MachineRoot::platform_profile_choices)
        .build()
        .expect("fake machine");

    let laptop = detect(&fake.machine());

    assert!(!laptop.has(Capability::ThermalProfile));
    assert!(laptop.thermal_profiles.is_empty());
}

#[test]
fn a_platform_profile_offering_no_choices_is_not_a_thermal_profile() {
    let fake = FakeMachine::builder()
        .predator_sense()
        .thermal_profile("balanced", "")
        .build()
        .expect("fake machine");

    assert!(!detect(&fake.machine()).has(Capability::ThermalProfile));
}

#[test]
fn the_turbo_state_is_read_when_the_laptop_has_a_turbo_key() {
    let engaged = FakeMachine::builder()
        .predator_legacy()
        .turbo(true)
        .build()
        .expect("fake machine");
    let plain = FakeMachine::builder()
        .predator_sense()
        .build()
        .expect("fake machine");

    assert_eq!(detect(&engaged.machine()).turbo, Some(Turbo::Engaged));
    assert_eq!(detect(&plain.machine()).turbo, None);
}

#[test]
fn a_legacy_predator_hides_attributes_its_firmware_rejects() {
    let fake = FakeMachine::builder()
        .predator_legacy()
        .build()
        .expect("fake machine");

    let laptop = detect(&fake.machine());

    assert_eq!(laptop.sense_interface, SenseInterface::Predator);
    assert_eq!(
        laptop.capabilities(),
        vec![
            Capability::ThermalProfile,
            Capability::Fans,
            Capability::UsbCharging,
            Capability::BootAnimation,
            Capability::LcdOverdrive,
            Capability::BacklightTimeout,
        ]
    );
}
