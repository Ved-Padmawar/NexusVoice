use tauri::{AppHandle, Emitter};

use crate::inference::provider::ModelSize;
use crate::state::ModelDownloadState;

/// Download the selected ggml model file. Reports progress via events.
pub fn download_whisper_model(
    models_dir: &std::path::Path,
    model_size: ModelSize,
    app: &AppHandle,
    dl_state: &ModelDownloadState,
) -> Result<(), String> {
    let file_urls: &[(&str, &str)] = &[(model_size.filename(), model_size.url())];

    // HEAD each URL to get Content-Length; use on-disk size for already-downloaded files
    let client = reqwest::blocking::Client::new();
    let mut file_sizes: Vec<u64> = Vec::new();
    for (filename, url) in file_urls {
        let dest = models_dir.join(filename);
        if dest.exists() {
            file_sizes.push(dest.metadata().map(|m| m.len()).unwrap_or(0));
        } else {
            let size = client
                .head(*url)
                .send()
                .ok()
                .and_then(|r| r.headers().get("content-length")?.to_str().ok()?.parse().ok())
                .unwrap_or(0);
            file_sizes.push(size);
        }
    }

    let total_bytes: u64 = file_sizes.iter().sum();
    let mut downloaded_total: u64 = 0;

    for ((filename, url), &size) in file_urls.iter().zip(file_sizes.iter()) {
        let dest = models_dir.join(filename);
        if dest.exists() {
            downloaded_total += size;
            continue;
        }
        download_file(url, &dest, app, dl_state, &mut downloaded_total, total_bytes)?;
    }

    Ok(())
}

fn download_file(
    url: &str,
    dest: &std::path::Path,
    app: &AppHandle,
    dl_state: &ModelDownloadState,
    downloaded_total: &mut u64,
    total_bytes: u64,
) -> Result<(), String> {
    use std::io::{Read, Write};

    let mut response =
        reqwest::blocking::get(url).map_err(|e| format!("download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("download HTTP {} for {}", response.status(), url));
    }

    let tmp = dest.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("create file failed: {e}"))?;

    let mut last_pct: u8 = 0;
    let mut buf = vec![0u8; 256 * 1024]; // 256 KB chunks
    loop {
        // Check cancel flag before each chunk — gives ~256 KB granularity.
        if dl_state.is_cancelled() {
            drop(file);
            let _ = std::fs::remove_file(&tmp); // clean up partial download
            return Err("download_cancelled".to_string());
        }
        let n = response
            .read(&mut buf)
            .map_err(|e| format!("read body failed: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("write file failed: {e}"))?;
        *downloaded_total += n as u64;
        if total_bytes > 0 {
            let pct = ((*downloaded_total * 100) / total_bytes).min(100) as u8;
            if pct != last_pct {
                last_pct = pct;
                dl_state.set_progress(pct);
                let _ = app.emit("model-download-progress", pct);
            }
        }
    }

    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| format!("rename file failed: {e}"))?;

    Ok(())
}
