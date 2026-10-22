//! The device list is what the user picks a recording input from. A duplicate
//! row, or a selection that points at the wrong device, records from the wrong
//! microphone.

use crate::api::InputDeviceInfo;

use super::{options, selected_value, DEFAULT_VALUE};

fn device(name: &str, is_default: bool, is_selected: bool) -> InputDeviceInfo {
    InputDeviceInfo {
        name: name.into(),
        is_default,
        is_selected,
    }
}

fn labels(devices: &[InputDeviceInfo]) -> Vec<String> {
    options(devices)
        .into_iter()
        .map(|(_, label)| label.to_string())
        .collect()
}

#[test]
fn the_default_device_is_only_listed_as_the_default_option() {
    let devices = [
        device("Realtek Microphone", true, false),
        device("Blue Yeti", false, false),
    ];
    assert_eq!(
        labels(&devices),
        ["Default — Realtek Microphone", "Blue Yeti"]
    );
    assert_eq!(options(&devices)[0].0, DEFAULT_VALUE);
}

#[test]
fn without_a_default_device_the_option_is_just_default() {
    assert_eq!(labels(&[]), ["Default"]);
    assert_eq!(
        labels(&[device("Blue Yeti", false, false)]),
        ["Default", "Blue Yeti"]
    );
}

#[test]
fn duplicate_and_default_prefixed_names_are_listed_once_or_not_at_all() {
    let devices = [
        device("Realtek Microphone", true, false),
        device("Blue Yeti", false, false),
        device("Blue Yeti", false, false),
        device("Default Audio Device", false, false),
    ];
    assert_eq!(
        labels(&devices),
        ["Default — Realtek Microphone", "Blue Yeti"]
    );
}

#[test]
fn a_saved_device_is_selected_and_otherwise_the_default_is() {
    let saved = [
        device("Realtek Microphone", true, false),
        device("Blue Yeti", false, true),
    ];
    assert_eq!(selected_value(&saved), "Blue Yeti");

    let none = [
        device("Realtek Microphone", true, false),
        device("Blue Yeti", false, false),
    ];
    assert_eq!(selected_value(&none), DEFAULT_VALUE);
}

#[test]
fn the_default_device_flagged_selected_still_means_follow_the_os() {
    // Saving the default device's own name must not pin it: the user would stop
    // following the OS default when it changes.
    let devices = [device("Realtek Microphone", true, true)];
    assert_eq!(selected_value(&devices), DEFAULT_VALUE);
}
