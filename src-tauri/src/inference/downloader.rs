//! Model file download: resumable, retried, cancellable.
//!
//! Runs on the async runtime and races each chunk against a `CancellationToken`,
//! so a cancel interrupts immediately. Bytes land in a `.part` file next to the
//! destination, letting an interrupted transfer resume with an HTTP Range
//! request instead of refetching gigabytes.

use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::inference::catalog::ModelEntry;
use crate::state::ModelDownloadState;

/// Error string the caller matches on to detect a user-initiated cancel.
pub const CANCELLED: &str = "download_cancelled";

/// Attempts per file before giving up; each retry resumes rather than restarts.
const MAX_ATTEMPTS: u32 = 4;
/// Backoff before attempt N is `RETRY_BASE_DELAY * 2^(N-1)` — 1s, 2s, 4s.
const RETRY_BASE_DELAY: Duration = Duration::from_secs(1);
/// Handshake only — a whole-request timeout would abort a healthy large
/// download mid-transfer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// No bytes for this long means the connection is dead; a slow but moving
/// download keeps going.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Download `entry` into `models_dir`, reporting progress via events.
/// Returns `Err(CANCELLED)` on user cancel so the caller can tell it from a
/// genuine failure.
pub async fn download_model(
    models_dir: &Path,
    entry: &ModelEntry,
    app: &AppHandle,
    dl_state: &ModelDownloadState,
) -> Result<(), String> {
    let dest = models_dir.join(&entry.filename);
    if dest.exists() {
        return Ok(());
    }

    let cancel = dl_state.cancel_token();
    let part = dest.with_extension("part");

    for attempt in 1..=MAX_ATTEMPTS {
        if cancel.is_cancelled() {
            return Err(CANCELLED.to_string());
        }

        match transfer(&entry.url, &part, app, dl_state, &cancel).await {
            Ok(()) => break,
            // Keep the .part file: the bytes let a later attempt resume, and
            // startup sweeps whatever is left behind.
            Err(e) if e == CANCELLED => return Err(CANCELLED.to_string()),
            Err(e) if attempt == MAX_ATTEMPTS => {
                return Err(format!("download failed after {attempt} attempts: {e}"));
            }
            Err(_) => {
                let backoff = RETRY_BASE_DELAY * 2u32.pow(attempt - 1);
                // Cancelling must not sit out the whole backoff.
                tokio::select! {
                    () = cancel.cancelled() => return Err(CANCELLED.to_string()),
                    () = tokio::time::sleep(backoff) => {}
                }
            }
        }
    }

    // A cancel landing during the last chunks must not publish the file.
    if cancel.is_cancelled() {
        return Err(CANCELLED.to_string());
    }

    tokio::fs::rename(&part, &dest)
        .await
        .map_err(|e| format!("rename file failed: {e}"))
}

/// Stream `url` into `part`, resuming from whatever is already there.
async fn transfer(
    url: &str,
    part: &Path,
    app: &AppHandle,
    dl_state: &ModelDownloadState,
    cancel: &CancellationToken,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|e| format!("http client failed: {e}"))?;

    let mut have = tokio::fs::metadata(part).await.map_or(0, |m| m.len());

    let mut request = client.get(url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("download request failed: {e}"))?;
    let status = response.status();

    // A server that ignores Range replies 200 with the whole file; start over so
    // appended bytes line up with the response.
    let resuming = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if have > 0 && !resuming {
        have = 0;
    }
    if !status.is_success() {
        return Err(format!("download HTTP {status} for {url}"));
    }

    let total = response.content_length().map_or(0, |len| len + have);

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resuming)
        .truncate(!resuming)
        .open(part)
        .await
        .map_err(|e| format!("create file failed: {e}"))?;

    let mut written = have;
    let mut last_pct = pct_of(written, total);
    let mut stream = response.bytes_stream();

    loop {
        let chunk = tokio::select! {
            () = cancel.cancelled() => {
                // Flush so the partial file stays a valid resume point.
                let _ = file.flush().await;
                return Err(CANCELLED.to_string());
            }
            next = stream.next() => match next {
                Some(chunk) => chunk.map_err(|e| format!("read body failed: {e}"))?,
                None => break,
            },
        };

        file.write_all(&chunk)
            .await
            .map_err(|e| format!("write file failed: {e}"))?;
        written += chunk.len() as u64;

        let pct = pct_of(written, total);
        if pct != last_pct {
            last_pct = pct;
            dl_state.set_progress(pct);
            let _ = app.emit("model-download-progress", pct);
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("flush file failed: {e}"))?;
    Ok(())
}

/// Percentage of `total` that `written` represents, saturating at 100. A `total`
/// of 0 (no Content-Length) reports 0 rather than dividing by zero.
fn pct_of(written: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    u8::try_from((written * 100 / total).min(100)).unwrap_or(100)
}

/// Delete leftover `.part` files from downloads interrupted by a crash or quit.
/// Called at startup, when nothing can be mid-transfer.
pub fn clean_stale_parts(models_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(models_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "part") {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/inference/downloader.rs"]
mod tests;
