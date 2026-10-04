//! Optional CUDA pack (ggml-cuda module + CUDA runtime), downloaded on demand
//! for NVIDIA GPUs. Downloads are staged and swapped in at the next start,
//! since loaded libraries cannot be replaced.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::inference::downloader::{download_file, CANCELLED};

const PACK_DIR: &str = "cuda";
const STAGING_DIR: &str = "cuda.next";
/// Written last, so its presence marks a complete pack.
const MANIFEST: &str = "manifest.json";
/// Asks the next start to delete a pack that is loaded and so locked now.
const REMOVE_MARKER: &str = "remove";

const RELEASES_URL: &str = "https://github.com/Ved-Padmawar/NexusVoice/releases/download";
/// The release this build shipped in; `release.yml` sets the version from the tag.
const RELEASE_TAG: &str = concat!("v", env!("CARGO_PKG_VERSION"));

/// The oldest GPU the pack has kernels for: `sm_75` (RTX 20 / GTX 16).
const MIN_COMPUTE_CAPABILITY: (i32, i32) = (7, 5);
/// The pack's runtime is CUDA 13; NVML reports it as major * 1000 + minor * 10.
const MIN_DRIVER_CUDA: i32 = 13_000;

#[cfg(target_os = "windows")]
const PLATFORM: Option<&str> = Some("windows-x86_64");
#[cfg(target_os = "linux")]
const PLATFORM: Option<&str> = Some("linux-x86_64");
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
const PLATFORM: Option<&str> = None;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// transcribe-cpp version the module was built from; it must match ours.
    pub engine: String,
    /// In load order: runtime libraries first, the ggml module last.
    pub files: Vec<PackFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFile {
    pub name: String,
    /// Release asset holding the gzipped file.
    pub asset: String,
    /// Of the unpacked file.
    pub sha256: String,
    /// Of the gzipped asset, for progress.
    pub size: u64,
}

/// A platform we build the pack for, with a GPU and driver NVML says can run it.
pub fn supported() -> bool {
    static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    PLATFORM.is_some()
        && *SUPPORTED.get_or_init(|| match query_nvml() {
            Ok((capability, driver)) => {
                log::info!("NVIDIA GPU: compute {capability:?}, driver CUDA {driver}");
                meets_requirements(capability, driver)
            }
            Err(e) => {
                log::info!("CUDA unavailable: {e}");
                false
            }
        })
}

fn meets_requirements(capability: (i32, i32), driver: i32) -> bool {
    capability >= MIN_COMPUTE_CAPABILITY && driver >= MIN_DRIVER_CUDA
}

/// The best GPU compute capability and the driver's CUDA version, from NVML.
fn query_nvml() -> Result<((i32, i32), i32), String> {
    use std::ffi::c_void;

    type Init = unsafe extern "C" fn() -> i32;
    type DriverVersion = unsafe extern "C" fn(*mut i32) -> i32;
    type Count = unsafe extern "C" fn(*mut u32) -> i32;
    type Handle = unsafe extern "C" fn(u32, *mut *mut c_void) -> i32;
    type Capability = unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32) -> i32;
    type Shutdown = unsafe extern "C" fn() -> i32;

    // SAFETY: NVML's documented C API; each symbol is called with its own signature.
    unsafe {
        let nvml = load_nvml()?;
        let get = |name: &str| format!("NVML lacks {name}");
        let init = nvml
            .get::<Init>(b"nvmlInit_v2")
            .map_err(|_| get("nvmlInit_v2"))?;
        let shutdown = nvml
            .get::<Shutdown>(b"nvmlShutdown")
            .map_err(|_| get("nvmlShutdown"))?;
        let driver_version = nvml
            .get::<DriverVersion>(b"nvmlSystemGetCudaDriverVersion_v2")
            .map_err(|_| get("nvmlSystemGetCudaDriverVersion_v2"))?;
        let count = nvml
            .get::<Count>(b"nvmlDeviceGetCount_v2")
            .map_err(|_| get("nvmlDeviceGetCount_v2"))?;
        let handle = nvml
            .get::<Handle>(b"nvmlDeviceGetHandleByIndex_v2")
            .map_err(|_| get("nvmlDeviceGetHandleByIndex_v2"))?;
        let capability = nvml
            .get::<Capability>(b"nvmlDeviceGetCudaComputeCapability")
            .map_err(|_| get("nvmlDeviceGetCudaComputeCapability"))?;

        if init() != 0 {
            return Err("NVML failed to initialise".to_string());
        }
        let mut driver = 0;
        let mut devices = 0;
        let mut best = None;
        if driver_version(&raw mut driver) == 0 && count(&raw mut devices) == 0 {
            for index in 0..devices {
                let mut device = std::ptr::null_mut();
                let (mut major, mut minor) = (0, 0);
                if handle(index, &raw mut device) == 0
                    && capability(device, &raw mut major, &raw mut minor) == 0
                {
                    best = best.max(Some((major, minor)));
                }
            }
        }
        shutdown();
        best.map(|capability| (capability, driver))
            .ok_or_else(|| "NVML found no NVIDIA GPU".to_string())
    }
}

/// Load NVML from where the driver installs it: System32 on current Windows
/// drivers, `NVSMI` on older ones.
fn load_nvml() -> Result<libloading::Library, String> {
    #[cfg(windows)]
    let candidates = {
        let program_files = std::env::var_os("ProgramW6432")
            .or_else(|| std::env::var_os("ProgramFiles"))
            .map_or_else(|| "C:\\Program Files".into(), std::path::PathBuf::from);
        [
            std::path::PathBuf::from("nvml.dll"),
            program_files
                .join("NVIDIA Corporation")
                .join("NVSMI")
                .join("nvml.dll"),
        ]
    };
    #[cfg(not(windows))]
    let candidates = [std::path::PathBuf::from("libnvidia-ml.so.1")];

    let mut last_error = String::new();
    for candidate in &candidates {
        // SAFETY: NVML's initialisers have no preconditions.
        match unsafe { libloading::Library::new(candidate) } {
            Ok(library) => return Ok(library),
            Err(e) => last_error = e.to_string(),
        }
    }
    Err(format!("no NVIDIA driver: {last_error}"))
}

/// A CUDA device registered at startup, so models will run on CUDA.
pub fn registered() -> bool {
    transcribe_cpp::devices()
        .iter()
        .any(|device| device.kind == "cuda")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PackState {
    Absent,
    /// Usable by this build; loaded unless the driver is missing or too old.
    Installed,
    /// Downloaded; applied at the next start.
    PendingInstall,
    /// Removed while loaded; deleted at the next start.
    PendingRemoval,
}

pub fn pack_state(app_data_dir: &Path) -> PackState {
    let dir = app_data_dir.join(PACK_DIR);
    if app_data_dir.join(STAGING_DIR).join(MANIFEST).exists() {
        PackState::PendingInstall
    } else if dir.join(REMOVE_MARKER).exists() {
        PackState::PendingRemoval
    } else if read_manifest(&dir).is_some_and(|m| matches_engine(&m)) {
        PackState::Installed
    } else {
        PackState::Absent
    }
}

/// Apply a staged download or removal, then register CUDA. Must run before the
/// bundled backends: ggml prefers whichever registers first.
pub fn activate(app_data_dir: &Path) {
    let dir = app_data_dir.join(PACK_DIR);
    let staging = app_data_dir.join(STAGING_DIR);

    if dir.join(REMOVE_MARKER).exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    if staging.join(MANIFEST).exists() {
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(e) = std::fs::rename(&staging, &dir) {
            log::warn!("CUDA pack: could not install the staged download: {e}");
        }
    }

    let Some(manifest) = read_manifest(&dir) else {
        return;
    };
    if !matches_engine(&manifest) {
        log::warn!(
            "CUDA pack built for engine {}, this build runs {}; download it again",
            manifest.engine,
            transcribe_cpp::version()
        );
        return;
    }

    // Preloaded so the module's by-name imports resolve to them.
    let (module, runtime) = manifest.files.split_last().expect("manifest is non-empty");
    for file in runtime {
        if let Err(e) = preload(&dir.join(&file.name)) {
            log::warn!("CUDA pack: could not load {}: {e}", file.name);
            return;
        }
    }
    match transcribe_cpp::init_backends(&dir) {
        Ok(()) => log::info!("CUDA pack: registered {}", module.name),
        // No driver, or one too old: the module is skipped and Vulkan serves.
        Err(e) => log::info!("CUDA pack: no CUDA device ({e})"),
    }
}

/// Delete the pack, or mark it for deletion at the next start when loaded.
pub fn remove(app_data_dir: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(app_data_dir.join(STAGING_DIR));
    let dir = app_data_dir.join(PACK_DIR);
    if !dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir).or_else(|_| File::create(dir.join(REMOVE_MARKER)).map(drop))
}

/// The one CUDA download allowed at a time, with its progress.
#[derive(Default)]
pub struct CudaDownload {
    cancel: Mutex<Option<CancellationToken>>,
    progress: AtomicU8,
}

impl CudaDownload {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<CancellationToken>> {
        self.cancel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Progress of the running download, or `None` when idle.
    pub fn progress(&self) -> Option<u8> {
        self.lock()
            .is_some()
            .then(|| self.progress.load(Ordering::Relaxed))
    }

    pub fn cancel(&self) {
        if let Some(token) = self.lock().as_ref() {
            token.cancel();
        }
    }

    /// Fetch the pack into the staging dir. `Err(CANCELLED)` on cancel; a
    /// download already running is left alone.
    pub async fn run(&self, app: &AppHandle, app_data_dir: &Path) -> Result<(), String> {
        let cancel = {
            let mut slot = self.lock();
            if slot.is_some() {
                return Ok(());
            }
            self.progress.store(0, Ordering::Relaxed);
            slot.insert(CancellationToken::new()).clone()
        };
        let result = self.fetch(app, app_data_dir, &cancel).await;
        *self.lock() = None;
        result
    }

    async fn fetch(
        &self,
        app: &AppHandle,
        app_data_dir: &Path,
        cancel: &CancellationToken,
    ) -> Result<(), String> {
        let platform = PLATFORM.ok_or("CUDA is not available on this platform")?;
        let manifest = fetch_manifest(platform).await?;
        if !matches_engine(&manifest) {
            return Err("the published CUDA pack does not match this version".to_string());
        }

        let dir = app_data_dir.join(PACK_DIR);
        let staging = app_data_dir.join(STAGING_DIR);
        std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        let current = read_manifest(&dir).filter(|_| !dir.join(REMOVE_MARKER).exists());

        let total: u64 = manifest.files.iter().map(|f| f.size).sum::<u64>().max(1);
        let mut done = 0;
        for file in &manifest.files {
            let dest = staging.join(&file.name);
            let unchanged = current.as_ref().is_some_and(|m| {
                m.files
                    .iter()
                    .any(|f| f.name == file.name && f.sha256 == file.sha256)
            });

            // Unchanged files (often the large runtime) are copied, not refetched.
            if unchanged {
                std::fs::copy(dir.join(&file.name), &dest).map_err(|e| e.to_string())?;
            } else if !verified(&dest, &file.sha256) {
                let gz = staging.join(format!("{}.gz", file.name));
                let url = format!("{RELEASES_URL}/{RELEASE_TAG}/{}", file.asset);
                download_file(&url, &gz, cancel, |pct| {
                    let overall = (done + file.size * u64::from(pct) / 100) * 100 / total;
                    self.report(app, overall);
                })
                .await?;
                let (gz, dest, sha256) = (gz.clone(), dest.clone(), file.sha256.clone());
                tokio::task::spawn_blocking(move || unpack(&gz, &dest, &sha256))
                    .await
                    .map_err(|e| e.to_string())??;
            }
            done += file.size;
            self.report(app, done * 100 / total);
        }

        if cancel.is_cancelled() {
            return Err(CANCELLED.to_string());
        }
        let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
        std::fs::write(staging.join(MANIFEST), json).map_err(|e| e.to_string())
    }

    fn report(&self, app: &AppHandle, pct: u64) {
        let pct = u8::try_from(pct.min(100)).unwrap_or(100);
        if self.progress.swap(pct, Ordering::Relaxed) != pct {
            let _ = app.emit("cuda-download-progress", pct);
        }
    }
}

async fn fetch_manifest(platform: &str) -> Result<Manifest, String> {
    let url = format!("{RELEASES_URL}/{RELEASE_TAG}/cuda-pack-{platform}.json");
    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("no CUDA pack is published for {RELEASE_TAG}"));
    }
    let manifest: Manifest = response
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| format!("bad CUDA pack manifest: {e}"))?;
    if manifest.files.is_empty() {
        return Err("the CUDA pack manifest lists no files".to_string());
    }
    Ok(manifest)
}

fn read_manifest(dir: &Path) -> Option<Manifest> {
    let text = std::fs::read(dir.join(MANIFEST)).ok()?;
    serde_json::from_slice::<Manifest>(&text)
        .ok()
        .filter(|m| !m.files.is_empty())
}

fn matches_engine(manifest: &Manifest) -> bool {
    manifest.engine == transcribe_cpp::version()
}

/// Gunzip `gz` into `dest`, keeping it only if it matches `sha256`.
fn unpack(gz: &Path, dest: &Path, sha256: &str) -> Result<(), String> {
    let tmp = dest.with_extension("tmp");
    let result = gunzip_hashed(gz, &tmp);
    let _ = std::fs::remove_file(gz);

    match result {
        Ok(hash) if hash == sha256 => std::fs::rename(&tmp, dest).map_err(|e| e.to_string()),
        Ok(_) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("checksum mismatch for {}", dest.display()))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("unpack {} failed: {e}", gz.display()))
        }
    }
}

fn gunzip_hashed(gz: &Path, out: &Path) -> std::io::Result<String> {
    let reader = flate2::read::GzDecoder::new(BufReader::new(File::open(gz)?));
    let mut writer = BufWriter::new(File::create(out)?);
    let hash = copy_hashed(reader, &mut writer)?;
    writer.flush()?;
    Ok(hash)
}

/// A file left by an interrupted download, already complete and intact.
fn verified(path: &Path, sha256: &str) -> bool {
    File::open(path)
        .and_then(|f| copy_hashed(BufReader::new(f), std::io::sink()))
        .is_ok_and(|hash| hash == sha256)
}

/// Copy `reader` into `writer`, returning the hex SHA-256 of the bytes.
fn copy_hashed(mut reader: impl Read, mut writer: impl Write) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        writer.write_all(&buf[..n])?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Load a library for the life of the process.
fn preload(path: &Path) -> Result<(), String> {
    // SAFETY: the CUDA runtime libraries run no initialisers with preconditions.
    #[cfg(unix)]
    let library = unsafe {
        use libloading::os::unix::{Library, RTLD_GLOBAL, RTLD_NOW};
        Library::open(Some(path), RTLD_NOW | RTLD_GLOBAL)
    };
    #[cfg(not(unix))]
    let library = unsafe { libloading::Library::new(path) };

    std::mem::forget(library.map_err(|e| e.to_string())?);
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/inference/cuda.rs"]
mod tests;
