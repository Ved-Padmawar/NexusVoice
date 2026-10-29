//! Reading the default sink's bus out of `pactl list sinks`.

use super::sink_is_bluetooth;

const SINKS: &str = r#"Sink #46
	State: SUSPENDED
	Name: alsa_output.pci-0000_00_1f.3.analog-stereo
	Properties:
		device.api = "alsa"
		device.bus = "pci"

Sink #71
	State: RUNNING
	Name: bluez_output.40_ED_98_1C_1D_08.1
	Properties:
		device.api = "bluez5"
		device.bus = "bluetooth"
		api.bluez5.address = "40:ED:98:1C:1D:08"
"#;

#[test]
fn a_bluez_sink_is_bluetooth() {
    assert_eq!(
        sink_is_bluetooth(SINKS, "bluez_output.40_ED_98_1C_1D_08.1"),
        Some(true)
    );
}

#[test]
fn a_built_in_sink_is_not_bluetooth() {
    assert_eq!(
        sink_is_bluetooth(SINKS, "alsa_output.pci-0000_00_1f.3.analog-stereo"),
        Some(false)
    );
}

#[test]
fn pulseaudio_api_marks_a_sink_without_a_bus() {
    let sinks =
        "Sink #3\n\tName: bluez_sink.40_ED.a2dp_sink\n\tProperties:\n\t\tdevice.api = \"bluez\"\n";
    assert_eq!(
        sink_is_bluetooth(sinks, "bluez_sink.40_ED.a2dp_sink"),
        Some(true)
    );
}

#[test]
fn an_unlisted_default_is_unknown() {
    assert_eq!(sink_is_bluetooth(SINKS, "missing"), None);
}
