use std::process::Command;

pub fn default_output_is_bluetooth() -> Option<bool> {
    let default = pactl(&["get-default-sink"])?;
    super::sink_is_bluetooth(&pactl(&["list", "sinks"])?, default.trim())
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
