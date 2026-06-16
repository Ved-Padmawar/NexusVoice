use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter};

use crate::inference::provider::ModelSize;
use crate::parakeet;
use crate::state::ModelDownloadState;

/// `HuggingFace` repo hosting the Parakeet-v3 ONNX export.
const PARAKEET_REPO: &str = "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main";
/// Files required by transcribe-rs's Int8 Parakeet loader.
const PARAKEET_FILES: [&str; 4] = [
    "encoder-model.int8.onnx",
    "decoder_joint-model.int8.onnx",
    "nemo128.onnx",
    "vocab.txt",
];

/// Download the selected ggml whisper model file. Reports progress via events.
pub fn download_whisper_model(
    models_dir: &Path,
    model_size: ModelSize,
    app: &AppHandle,
    dl_state: &ModelDownloadState,
) -> Result<(), String> {
    download_set(
        &[(models_dir.join(model_size.filename()), model_size.url().to_string())],
        app,
        dl_state,
    )
}

/// Download the Parakeet ONNX model set into its subdirectory.
pub fn download_parakeet_model(
    models_dir: &Path,
    app: &AppHandle,
    dl_state: &ModelDownloadState,
) -> Result<(), String> {
    let dir = models_dir.join(parakeet::MODEL_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create model dir failed: {e}"))?;

    let files: Vec<(PathBuf, String)> = PARAKEET_FILES
        .iter()
        .map(|f| (dir.join(f), format!("{PARAKEET_REPO}/{f}")))
        .collect();

    download_set(&files, app, dl_state)
}

/// Download a set of (destination, url) pairs as one logical model, reporting
/// aggregate progress. Already-present files are skipped.
fn download_set(
    files: &[(PathBuf, String)],
    app: &AppHandle,
    dl_state: &ModelDownloadState,
) -> Result<(), String> {
    let client = reqwest::blocking::Client::new();
    let mut file_sizes: Vec<u64> = Vec::with_capacity(files.len());
    for (dest, url) in files {
        if dest.exists() {
            file_sizes.push(dest.metadata().map(|m| m.len()).unwrap_or(0));
        } else {
            let size = client
                .head(url)
                .send()
                .ok()
                .and_then(|r| {
                    r.headers()
                        .get("content-length")?
                        .to_str()
                        .ok()?
                        .parse()
                        .ok()
                })
                .unwrap_or(0);
            file_sizes.push(size);
        }
    }

    let total_bytes: u64 = file_sizes.iter().sum();
    let mut downloaded_total: u64 = 0;

    for ((dest, url), &size) in files.iter().zip(file_sizes.iter()) {
        if dest.exists() {
            downloaded_total += size;
            continue;
        }
        download_file(url, dest, app, dl_state, &mut downloaded_total, total_bytes)?;
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
