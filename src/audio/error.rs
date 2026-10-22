//! Turns cpal/OS capture errors into messages that tell the user what to do.
//!
//! cpal surfaces failures as opaque, platform-specific strings ("Access is
//! denied. (os error 5)", "device is no longer available", …). Matching on their
//! recognizable fragments lets us name the likely cause — permission, another app
//! holding the mic, a disconnected device — instead of showing raw text.

/// Rewrite a raw capture error into an actionable, user-facing message. Unknown
/// errors pass through with a generic prefix so nothing is ever swallowed.
pub fn friendly_capture_error(raw: &str) -> String {
    let lower = raw.to_lowercase();

    if lower.contains("no input device") {
        return "No microphone was found. Connect one and try again.".to_string();
    }

    // Windows privacy toggle / OS-level permission denial.
    if lower.contains("access is denied")
        || lower.contains("permission")
        || lower.contains("os error 5")
    {
        return "NexusVoice can't access your microphone. Check that microphone \
                access is enabled in your system's privacy settings."
            .to_string();
    }

    // Device grabbed in exclusive mode, or gone mid-session.
    if lower.contains("in use")
        || lower.contains("exclusive")
        || lower.contains("device is no longer valid")
        || lower.contains("already in use")
    {
        return "Another app is using your microphone exclusively. Close apps like \
                Teams, Zoom, or Discord and try again."
            .to_string();
    }

    if lower.contains("no longer available")
        || lower.contains("disconnected")
        || lower.contains("not available")
        || lower.contains("device unavailable")
    {
        return "Your microphone was disconnected. Reconnect it and try again.".to_string();
    }

    if lower.contains("format") || lower.contains("sample") || lower.contains("config") {
        return "Your microphone reported an unsupported audio format. Try a \
                different input device in Settings."
            .to_string();
    }

    format!("Microphone error: {raw}")
}

#[cfg(test)]
#[path = "../../tests/unit/audio/error.rs"]
mod tests;
