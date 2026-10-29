use std::process::Command;

/// The desktop can restore a stream to Bluetooth while the default sink stays
/// on the speakers, so this reads the sink this process actually plays on.
pub fn playback_is_bluetooth() -> Option<bool> {
    let sink_inputs = pactl(&["list", "sink-inputs"])?;
    let sink = super::sink_of_process(&sink_inputs, std::process::id())?;
    super::sink_is_bluetooth(&pactl(&["list", "sinks"])?, sink)
}

/// `LC_ALL=C`: the field labels `pactl` prints are translated.
fn pactl(args: &[&str]) -> Option<String> {
    let output = Command::new("pactl")
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .inspect_err(|e| log::debug!("pactl unavailable: {e}"))
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
