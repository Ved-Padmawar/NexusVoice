//! File download for models and the CUDA pack: resumable, retried, cancellable.
//!
//! Runs on the async runtime and races each chunk against a `CancellationToken`,
//! so a cancel interrupts immediately. Bytes land in a `.part` file next to the
//! destination, letting an interrupted transfer resume with an HTTP Range
//! request instead of refetching gigabytes.

use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::bridge::events::ModelDownloadProgress;
use crate::models::catalog::{self, ModelEntry};
use crate::models::library::Downloads;

/// Error string the caller matches on to detect a user-initiated cancel.
pub const CANCELLED: &str = "download_cancelled";

const LOCK_FILE: &str = ".downloads.lock";

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
    downloads: &Downloads,
    cancel: CancellationToken,
) -> Result<(), String> {
    let dest = models_dir.join(&entry.filename);
    if dest.exists() {
        return Ok(());
    }

    let _lock = DownloadLock::acquire(models_dir, &entry.filename)?;
    let id = entry.id.as_str();
    download_file(&entry.url, &dest, &cancel, entry.size_bytes, |pct| {
        downloads.set_progress(id, pct);
        let _ = ModelDownloadProgress {
            id: id.to_string(),
            pct,
        }
        .emit(app);
    })
    .await
}

/// Download `url` to `dest`, calling `on_progress` with each new percentage.
/// `size` is the exact expected length; anything longer is refused.
/// Returns `Err(CANCELLED)` on cancel.
pub async fn download_file(
    url: &str,
    dest: &Path,
    cancel: &CancellationToken,
    size: u64,
    mut on_progress: impl FnMut(u8),
) -> Result<(), String> {
    let part = dest.with_extension("part");

    if cancel.is_cancelled() {
        return Err(CANCELLED.to_string());
    }
    let mut have = tokio::fs::metadata(&part).await.map_or(0, |m| m.len());
    if have > size {
        tokio::fs::remove_file(&part)
            .await
            .map_err(|e| describe_io(&e))?;
        have = 0;
    }
    // A complete partial needs no HTTP range request at EOF.
    if have != size {
        ensure_free_space(dest, size - have)?;
        for attempt in 1..=MAX_ATTEMPTS {
            if cancel.is_cancelled() {
                return Err(CANCELLED.to_string());
            }

            match transfer(url, &part, cancel, size, &mut on_progress).await {
                Ok(()) => break,
                // Keep the .part file: the bytes let a later attempt, or the next
                // launch, resume.
                Err(e) if e == CANCELLED => return Err(CANCELLED.to_string()),
                Err(e) if attempt == MAX_ATTEMPTS => return Err(e),
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
    }

    if tokio::fs::metadata(&part)
        .await
        .map_err(|e| describe_io(&e))?
        .len()
        != size
    {
        let _ = tokio::fs::remove_file(&part).await;
        return Err("The download was incomplete. Try again.".to_string());
    }

    // A cancel landing during the last chunks must not publish the file.
    if cancel.is_cancelled() {
        return Err(CANCELLED.to_string());
    }

    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| describe_io(&e))
}

/// Shown when a response carries more bytes than the catalog says.
const TOO_LARGE: &str = "The download is larger than expected. Try again later.";

/// Stream `url` into `part`, resuming from whatever is already there.
async fn transfer(
    url: &str,
    part: &Path,
    cancel: &CancellationToken,
    size: u64,
    on_progress: &mut impl FnMut(u8),
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|e| describe_request_error(&e))?;

    let mut have = tokio::fs::metadata(part).await.map_or(0, |m| m.len());

    let mut request = client.get(url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }

    // A stalled server holds `send` for up to the connect + read timeouts.
    let response = tokio::select! {
        () = cancel.cancelled() => return Err(CANCELLED.to_string()),
        response = request.send() => {
            response.map_err(|e| describe_request_error(&e))?
        }
    };
    let status = response.status();

    // A server that ignores Range replies 200 with the whole file; start over so
    // appended bytes line up with the response.
    let resuming = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if resuming && range_start(&response) != Some(have) {
        // Appending bytes from another offset would corrupt the file.
        let _ = tokio::fs::remove_file(part).await;
        return Err("The download server resumed from the wrong place. Try again.".to_string());
    }
    if have > 0 && !resuming {
        have = 0;
    }
    if !status.is_success() {
        return Err(describe_status(status));
    }

    let total = response
        .content_length()
        .map_or(0, |len| len.saturating_add(have));
    if total > size {
        return Err(TOO_LARGE.to_string());
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resuming)
        .truncate(!resuming)
        .open(part)
        .await
        .map_err(|e| describe_io(&e))?;

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
                Some(chunk) => chunk.map_err(|e| describe_request_error(&e))?,
                None => break,
            },
        };

        if written.saturating_add(chunk.len() as u64) > size {
            return Err(TOO_LARGE.to_string());
        }
        file.write_all(&chunk).await.map_err(|e| describe_io(&e))?;
        written += chunk.len() as u64;

        let pct = pct_of(written, total);
        if pct != last_pct {
            last_pct = pct;
            on_progress(pct);
        }
    }

    file.flush().await.map_err(|e| describe_io(&e))?;
    Ok(())
}

/// A plain sentence for a failed request, with the technical cause (which
/// carries the full URL) kept in the log.
pub fn describe_request_error(error: &reqwest::Error) -> String {
    log::warn!("download request failed: {error}");
    match error.status() {
        Some(status) => describe_status(status),
        None => "Couldn't reach the download server. Check your connection.".to_string(),
    }
}

fn describe_status(status: reqwest::StatusCode) -> String {
    format!(
        "The download server answered with an error ({}).",
        status.as_u16()
    )
}

/// A plain sentence for a failed file operation; a full disk is the one the
/// user can act on.
pub fn describe_io(error: &std::io::Error) -> String {
    log::warn!("download file operation failed: {error}");
    // std maps ERROR_DISK_FULL (112) and ENOSPC to `StorageFull`.
    if error.kind() == std::io::ErrorKind::StorageFull {
        "Your disk is full.".to_string()
    } else {
        "Couldn't save the download to disk.".to_string()
    }
}

/// Fail before transferring when the destination volume cannot hold `needed` more bytes.
fn ensure_free_space(dest: &Path, needed: u64) -> Result<(), String> {
    let Some(free) = free_space(dest) else {
        return Ok(());
    };
    if free < needed {
        return Err(format!(
            "Not enough disk space: needs {}, {} free.",
            format_size(needed),
            format_size(free)
        ));
    }
    Ok(())
}

/// Free bytes on the volume holding `path`, or `None` when it cannot be told.
fn free_space(path: &Path) -> Option<u64> {
    sysinfo::Disks::new_with_refreshed_list()
        .list()
        .iter()
        .filter(|disk| is_on_mount(path, disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count())
        .map(sysinfo::Disk::available_space)
}

/// Whole components, so `/home-backup` is not on `/home`.
fn is_on_mount(path: &Path, mount: &Path) -> bool {
    let mut parts = path.components();
    mount.components().all(|m| {
        parts.next().is_some_and(|p| {
            // Windows drive and folder names ignore case; Unix ones do not.
            if cfg!(windows) {
                p.as_os_str().eq_ignore_ascii_case(m.as_os_str())
            } else {
                p == m
            }
        })
    })
}

#[allow(clippy::cast_precision_loss)] // sizes are shown to one decimal
fn format_size(bytes: u64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else {
        format!("{:.0} MB", bytes / MB)
    }
}

/// First byte of the range a 206 response carries.
fn range_start(response: &reqwest::Response) -> Option<u64> {
    let header = response.headers().get(reqwest::header::CONTENT_RANGE)?;
    parse_range_start(header.to_str().ok()?)
}

/// The `100` in `bytes 100-199/200`.
fn parse_range_start(header: &str) -> Option<u64> {
    header
        .strip_prefix("bytes ")?
        .split('-')
        .next()?
        .parse()
        .ok()
}

/// Percentage of `total` that `written` represents, saturating at 100. A `total`
/// of 0 (no Content-Length) reports 0 rather than dividing by zero.
fn pct_of(written: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    u8::try_from((written * 100 / total).min(100)).unwrap_or(100)
}

/// Held for a download's length. Shared on the folder, so downloads overlap and
/// other builds' startup sweeps stay out; exclusive on the model, so two
/// processes never write the same `.part`.
pub struct DownloadLock {
    _folder: std::fs::File,
    _model: std::fs::File,
}

impl DownloadLock {
    pub fn acquire(models_dir: &Path, filename: &str) -> Result<Self, String> {
        let open = |name: &str| open_lock_file(&models_dir.join(name)).map_err(|e| describe_io(&e));
        let folder = open(LOCK_FILE)?;
        folder
            .try_lock_shared()
            .map_err(|_| "The models folder is busy. Try again in a moment.".to_string())?;
        let model = open(&format!("{filename}.lock"))?;
        model
            .try_lock()
            .map_err(|_| "Another NexusVoice is already downloading this model.".to_string())?;
        Ok(Self {
            _folder: folder,
            _model: model,
        })
    }
}

fn open_lock_file(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
}

/// Delete model lock files left by a crash or quit, and `.part` files that no
/// catalog model would resume. Skipped while any process is mid-download.
pub fn clean_stale_parts(models_dir: &Path) {
    let Ok(lock) = open_lock_file(&models_dir.join(LOCK_FILE)) else {
        return;
    };
    if lock.try_lock().is_err() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(models_dir) else {
        return;
    };
    let resumable: Vec<std::ffi::OsString> = catalog::all()
        .iter()
        .map(|model| {
            Path::new(&model.filename)
                .with_extension("part")
                .into_os_string()
        })
        .collect();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let stale = match path.extension().and_then(|ext| ext.to_str()) {
            Some("lock") => name != LOCK_FILE,
            Some("part") => !resumable.contains(&name),
            _ => false,
        };
        if stale {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/models/downloader.rs"]
mod tests;
