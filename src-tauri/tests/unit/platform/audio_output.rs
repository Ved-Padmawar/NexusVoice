//! Finding where this process plays, in `pactl list` output.

use super::{sink_is_bluetooth, sink_of_process};

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

const SINK_INPUTS: &str = r#"Sink Input #112
	Driver: protocol-native.c
	Sink: 46
	Properties:
		application.name = "Firefox"
		application.process.id = "2210"

Sink Input #118
	Driver: protocol-native.c
	Sink: 71
	Properties:
		application.name = "ALSA plug-in [nexusvoice]"
		application.process.id = "4031"
"#;

#[test]
fn finds_the_sink_a_process_plays_on() {
    assert_eq!(sink_of_process(SINK_INPUTS, 4031), Some("71"));
    assert_eq!(sink_of_process(SINK_INPUTS, 2210), Some("46"));
}

#[test]
fn a_process_without_a_stream_has_no_sink() {
    assert_eq!(sink_of_process(SINK_INPUTS, 9999), None);
}

#[test]
fn a_bluez_sink_is_bluetooth() {
    assert_eq!(sink_is_bluetooth(SINKS, "71"), Some(true));
}

#[test]
fn a_built_in_sink_is_not_bluetooth() {
    assert_eq!(sink_is_bluetooth(SINKS, "46"), Some(false));
}

#[test]
fn pulseaudio_api_marks_a_sink_without_a_bus() {
    let sinks =
        "Sink #3\n\tName: bluez_sink.40_ED.a2dp_sink\n\tProperties:\n\t\tdevice.api = \"bluez\"\n";
    assert_eq!(sink_is_bluetooth(sinks, "3"), Some(true));
}

#[test]
fn an_unlisted_sink_is_unknown() {
    assert_eq!(sink_is_bluetooth(SINKS, "5"), None);
}
