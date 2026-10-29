//! Whether the default audio output is Bluetooth. A Bluetooth sink drops the
//! start of a stream it was just woken for; a wired or built-in one does not.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// `None` when the platform cannot tell.
pub fn default_output_is_bluetooth() -> Option<bool> {
    #[cfg(windows)]
    return windows::default_output_is_bluetooth();
    #[cfg(target_os = "linux")]
    return linux::default_output_is_bluetooth();
    #[cfg(not(any(windows, target_os = "linux")))]
    None
}

/// Reads `pactl list sinks`, which `PulseAudio` and `PipeWire` both answer. Both
/// set `device.bus = "bluetooth"` on their Bluetooth sinks.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn sink_is_bluetooth(sinks: &str, name: &str) -> Option<bool> {
    let block = sinks.split("\nSink #").find(|block| {
        block
            .lines()
            .any(|line| line.trim() == format!("Name: {name}"))
    })?;
    Some(block.lines().any(|line| {
        let line = line.trim();
        line == r#"device.bus = "bluetooth""# || line.starts_with(r#"device.api = "bluez"#)
    }))
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/audio_output.rs"]
mod tests;
