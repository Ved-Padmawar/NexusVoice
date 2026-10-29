//! Whether this app's sound plays on Bluetooth. A Bluetooth sink drops the
//! start of a stream it was just woken for; a wired or built-in one does not.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// Call with a playback stream open. `None` when the platform cannot tell.
pub fn playback_is_bluetooth() -> Option<bool> {
    #[cfg(windows)]
    return windows::default_output_is_bluetooth();
    #[cfg(target_os = "linux")]
    return linux::playback_is_bluetooth();
    #[cfg(not(any(windows, target_os = "linux")))]
    None
}

/// `pactl list` output, split into its numbered blocks: `(index, body)`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn blocks<'a>(list: &'a str, header: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
    list.split(header).skip(1).map(|block| {
        block
            .split_once('\n')
            .map_or((block.trim(), ""), |(index, body)| (index.trim(), body))
    })
}

/// The sink a process's playback stream is on, from `pactl list sink-inputs`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn sink_of_process(sink_inputs: &str, pid: u32) -> Option<&str> {
    let owner = format!(r#"application.process.id = "{pid}""#);
    blocks(sink_inputs, "Sink Input #")
        .map(|(_, body)| body)
        .find(|body| body.lines().any(|line| line.trim() == owner))?
        .lines()
        .find_map(|line| line.trim().strip_prefix("Sink: "))
}

/// From `pactl list sinks`. `PulseAudio` and `PipeWire` both mark their
/// Bluetooth sinks with `device.bus = "bluetooth"`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn sink_is_bluetooth(sinks: &str, sink: &str) -> Option<bool> {
    let (_, body) = blocks(sinks, "Sink #").find(|(index, _)| *index == sink)?;
    Some(body.lines().any(|line| {
        let line = line.trim();
        line == r#"device.bus = "bluetooth""# || line.starts_with(r#"device.api = "bluez"#)
    }))
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/audio_output.rs"]
mod tests;
