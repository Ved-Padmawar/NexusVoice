//! Self-update from the GitHub release feed.
//!
//! Reads the same `latest.json` and verifies the same minisign signatures the
//! Tauri build used, so an installed copy keeps updating across the switch.
//! Windows runs the downloaded installer; a Linux AppImage replaces itself.
//! deb and rpm installs update through their package manager.

use std::path::PathBuf;

use base64::Engine as _;
use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};

const ENDPOINT: &str =
    "https://github.com/Ved-Padmawar/NexusVoice/releases/latest/download/latest.json";

/// The release signing key, base64 of the minisign public key file.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDhBOTlFRDAyRUNDQUVDRUEKUldUcTdNcnNBdTJaaWhydmo0cmIwRzRXNk1aUEdsRU04VjZzaXVKZG9peG8wU0cvRHlmQVhUUGcK";

#[cfg(target_os = "windows")]
const PLATFORM: &str = "windows-x86_64";
#[cfg(not(target_os = "windows"))]
const PLATFORM: &str = "linux-x86_64";

#[derive(serde::Deserialize)]
struct Manifest {
    version: String,
    platforms: std::collections::HashMap<String, Artifact>,
}

#[derive(serde::Deserialize)]
struct Artifact {
    signature: String,
    url: String,
}

/// A newer release, ready to download.
#[derive(Debug, Clone)]
pub struct Update {
    pub version: String,
    url: String,
    signature: String,
}

/// The newest release, or `None` when this build is current.
///
/// # Errors
/// The feed is unreachable or malformed, or has no build for this platform.
pub async fn check() -> Result<Option<Update>, String> {
    let manifest: Manifest = reqwest::get(ENDPOINT)
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    newer_update(&manifest, env!("CARGO_PKG_VERSION"), PLATFORM)
}

/// The update `manifest` offers over `current` for `platform`, if any.
fn newer_update(manifest: &Manifest, current: &str, platform: &str) -> Result<Option<Update>, String> {
    let latest = semver::Version::parse(manifest.version.trim_start_matches('v'))
        .map_err(|e| format!("bad version in update feed: {e}"))?;
    let current = semver::Version::parse(current).map_err(|e| e.to_string())?;
    if latest <= current {
        return Ok(None);
    }
    let artifact = manifest
        .platforms
        .get(platform)
        .ok_or_else(|| format!("no {platform} build in this release"))?;
    Ok(Some(Update {
        version: latest.to_string(),
        url: artifact.url.clone(),
        signature: artifact.signature.clone(),
    }))
}

/// Download, verify and install `update`, reporting whole percentages.
/// On Windows the installer takes over and this process should exit.
///
/// # Errors
/// The download fails, the signature does not verify, or installing fails.
pub async fn install(update: &Update, mut on_progress: impl FnMut(u8)) -> Result<(), String> {
    let target = install_target()?;
    let response = reqwest::get(&update.url)
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| e.to_string())?;
    let total = response.content_length().unwrap_or(0);
    let mut bytes = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
    let mut stream = response.bytes_stream();
    let mut reported = 0;
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        if total > 0 {
            let pct = u8::try_from(bytes.len() as u64 * 100 / total).unwrap_or(100);
            if pct != reported {
                reported = pct;
                on_progress(pct);
            }
        }
    }

    verify(&bytes, &update.signature, PUBLIC_KEY)?;
    tokio::task::spawn_blocking(move || apply(&target, &bytes))
        .await
        .map_err(|e| e.to_string())?
}

/// Start a fresh copy of the app. The caller quits right after.
///
/// # Errors
/// The executable cannot be found or started.
pub fn relaunch() -> Result<(), String> {
    let exe = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .map_or_else(std::env::current_exe, Ok)
        .map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// Check `bytes` against a base64 minisign `signature` and `public_key`, both
/// as the release tooling writes them.
fn verify(bytes: &[u8], signature: &str, public_key: &str) -> Result<(), String> {
    let decode = |b64: &str| -> Result<String, String> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| e.to_string())?;
        String::from_utf8(raw).map_err(|e| e.to_string())
    };
    let key = decode(public_key)?;
    let key = PublicKey::decode(&key).map_err(|e| e.to_string())?;
    let signature = Signature::decode(&decode(signature)?).map_err(|e| e.to_string())?;
    key.verify(bytes, &signature, false)
        .map_err(|_| "the update's signature did not verify".to_string())
}

/// Where the verified download goes.
#[cfg(target_os = "windows")]
fn install_target() -> Result<PathBuf, String> {
    Ok(std::env::temp_dir().join("NexusVoice-update-setup.exe"))
}

/// Only an AppImage can replace itself; its path is in `$APPIMAGE`.
#[cfg(not(target_os = "windows"))]
fn install_target() -> Result<PathBuf, String> {
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .ok_or_else(|| "this install updates through your package manager".to_string())
}

/// Run the installer passively; it closes this app and restarts it when done.
#[cfg(target_os = "windows")]
fn apply(target: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(target, bytes).map_err(|e| e.to_string())?;
    std::process::Command::new(target)
        .args(["/P", "/R"])
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// Swap the AppImage in place: written beside it, then renamed over it, so a
/// failure leaves the old one intact. Takes effect on restart.
#[cfg(not(target_os = "windows"))]
fn apply(target: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let staged = target.with_extension("update");
    std::fs::write(&staged, bytes).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    std::fs::rename(&staged, target).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "../../tests/unit/platform/updater.rs"]
mod tests;
