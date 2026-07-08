use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use tauri::{AppHandle, Emitter};

use crate::inference::provider::Model;
use crate::state::ModelDownloadState;

/// How many times to resume an interrupted transfer before giving up.
const MAX_ATTEMPTS: u32 = 5;
/// Backoff between resume attempts.
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// Download one GGUF atomically, resuming across interruptions. A partial file
/// is never exposed as a model: the transfer accumulates in a `.part` file and
/// is only renamed into place once its length matches the expected size.
pub fn download_model(
    models_dir: &Path,
    model: Model,
    app: &AppHandle,
    state: &ModelDownloadState,
) -> Result<(), String> {
    std::fs::create_dir_all(models_dir).map_err(|e| format!("create model directory: {e}"))?;
    let destination = models_dir.join(model.filename());
    let expected = model.size_bytes();
    if destination.metadata().is_ok_and(|m| m.len() == expected) {
        return Ok(());
    }

    let temporary = destination.with_extension("gguf.part");
    // A leftover `.part` from a previous run that already matches or overshoots
    // the expected size is unusable for resume — start clean.
    if temporary
        .metadata()
        .is_ok_and(|m| m.len() >= expected)
    {
        let _ = std::fs::remove_file(&temporary);
    }

    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        if state.is_cancelled() {
            let _ = std::fs::remove_file(&temporary);
            return Err("download_cancelled".into());
        }
        match download_attempt(&temporary, model, expected, app, state) {
            Ok(()) => {
                install(&temporary, &destination, expected, model)?;
                return Ok(());
            }
            Err(DownloadError::Cancelled) => {
                let _ = std::fs::remove_file(&temporary);
                return Err("download_cancelled".into());
            }
            // A fatal HTTP/server error won't be cured by resuming.
            Err(DownloadError::Fatal(message)) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(message);
            }
            // A dropped connection is retryable: keep the `.part` and resume.
            Err(DownloadError::Interrupted(message)) => {
                last_error = message;
                if attempt < MAX_ATTEMPTS {
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }
    }
    let _ = std::fs::remove_file(&temporary);
    Err(format!(
        "download failed after {MAX_ATTEMPTS} attempts: {last_error}"
    ))
}

enum DownloadError {
    /// User cancelled mid-transfer.
    Cancelled,
    /// Retrying can't help (bad status, cannot open file).
    Fatal(String),
    /// Connection dropped mid-stream; resume from the current `.part` length.
    Interrupted(String),
}

/// Perform (or resume) a single transfer into `temporary`. On success the file
/// is exactly `expected` bytes long.
fn download_attempt(
    temporary: &Path,
    model: Model,
    expected: u64,
    app: &AppHandle,
    state: &ModelDownloadState,
) -> Result<(), DownloadError> {
    // Resume from whatever we already have on disk.
    let mut already = temporary.metadata().map_or(0, |m| m.len());
    if already > expected {
        // Corrupt/oversized partial — discard and restart this attempt clean.
        let _ = std::fs::remove_file(temporary);
        already = 0;
    }
    if already == expected {
        return Ok(());
    }

    let mut request = reqwest::blocking::Client::new().get(model.url());
    if already > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={already}-"));
    }
    let mut response = request
        .send()
        .map_err(|e| DownloadError::Interrupted(format!("download request failed: {e}")))?;

    let status = response.status();
    // 206 = server honoured our Range resume; 200 = full body from the start
    // (server ignored Range), so we must overwrite from byte 0.
    let resuming = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if !status.is_success() {
        return Err(DownloadError::Fatal(format!(
            "model download returned HTTP {status}"
        )));
    }
    if already > 0 && !resuming {
        already = 0; // server sent the whole file; restart from scratch
    }

    let mut file = if resuming {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(temporary)
            .map_err(|e| DownloadError::Fatal(format!("open temporary model file: {e}")))?;
        f.seek(SeekFrom::Start(already))
            .map_err(|e| DownloadError::Fatal(format!("seek temporary model file: {e}")))?;
        f
    } else {
        std::fs::File::create(temporary)
            .map_err(|e| DownloadError::Fatal(format!("create temporary model file: {e}")))?
    };

    let mut downloaded = already;
    let mut last_progress = u8::try_from((already.saturating_mul(100) / expected).min(100))
        .unwrap_or(0);
    let mut buffer = vec![0_u8; 256 * 1024];

    loop {
        if state.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let count = response
            .read(&mut buffer)
            .map_err(|e| DownloadError::Interrupted(format!("read model download: {e}")))?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|e| DownloadError::Fatal(format!("write model download: {e}")))?;
        downloaded = downloaded.saturating_add(count as u64);
        let progress =
            u8::try_from((downloaded.saturating_mul(100) / expected).min(100)).unwrap_or(100);
        if progress != last_progress {
            last_progress = progress;
            state.set_progress(progress);
            let _ = app.emit("model-download-progress", progress);
        }
    }
    file.sync_all()
        .map_err(|e| DownloadError::Fatal(format!("flush model file: {e}")))?;
    drop(file);

    // The stream ended. If it ended short, the connection was cut mid-transfer;
    // report it as interrupted so the caller resumes rather than deleting.
    if downloaded < expected {
        return Err(DownloadError::Interrupted(format!(
            "connection closed early: have {downloaded} of {expected} bytes"
        )));
    }
    Ok(())
}

/// Validate the completed `.part` and atomically move it into place.
fn install(
    temporary: &Path,
    destination: &Path,
    expected: u64,
    model: Model,
) -> Result<(), String> {
    let actual = temporary
        .metadata()
        .map_err(|e| format!("stat model file: {e}"))?
        .len();
    if actual != expected {
        let _ = std::fs::remove_file(temporary);
        return Err(format!(
            "model size mismatch: expected {}, received {actual}",
            model.size_bytes()
        ));
    }
    if destination.exists() {
        std::fs::remove_file(destination).map_err(|e| format!("replace model file: {e}"))?;
    }
    std::fs::rename(temporary, destination).map_err(|e| format!("install model file: {e}"))
}
