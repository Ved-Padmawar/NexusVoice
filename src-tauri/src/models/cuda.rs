//! Optional CUDA pack (ggml-cuda module + CUDA runtime), downloaded on demand
//! for NVIDIA GPUs, loaded by the engine workers. Downloads are staged and
//! swapped in at the next start, since loaded libraries cannot be replaced.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

use crate::bridge::events::{
    CudaDownloadCancelled, CudaDownloadComplete, CudaDownloadError, CudaDownloadProgress,
    CudaDownloadVerifying,
};
use crate::models::downloader::{describe_io, describe_request_error, download_file, CANCELLED};
use crate::sync::lock_recovering;

const PACK_DIR: &str = "cuda";
const STAGING_DIR: &str = "cuda.next";
/// Written last, so its presence marks a complete pack.
const MANIFEST: &str = "manifest.json";
const SIGNATURE: &str = "manifest.json.sig";
/// Size and modified time of each file as of its last full hash check. A start
/// trusts files that still match it instead of re-hashing ~1 GB.
const STAMPS: &str = "verified.json";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024;
const MAX_LIBRARY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
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
    pub platform: String,
    pub release: String,
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
    pub unpacked_size: u64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PackState {
    Absent,
    /// Unsigned, so never loaded: installed before verification existed.
    Unverified,
    /// Usable by this build; loaded unless the driver is missing or too old.
    Installed,
    /// Signed and current, but a library failed its integrity check at start, so
    /// it was not loaded. Downloading again repairs it.
    Damaged,
    /// Downloaded; applied at the next start.
    PendingInstall,
    /// Removed while loaded; deleted at the next start.
    PendingRemoval,
}

pub fn pack_state(app_data_dir: &Path) -> PackState {
    let dir = app_data_dir.join(PACK_DIR);
    if read_manifest(&app_data_dir.join(STAGING_DIR)).is_some_and(|m| matches_engine(&m)) {
        PackState::PendingInstall
    } else if dir.join(REMOVE_MARKER).exists() {
        PackState::PendingRemoval
    } else {
        match read_manifest(&dir) {
            Some(manifest) if matches_engine(&manifest) => PackState::Installed,
            None if dir.join(MANIFEST).exists() => PackState::Unverified,
            _ => PackState::Absent,
        }
    }
}

/// Apply a staged download or removal, at app start. Returns whether the
/// installed pack failed its integrity check, so workers leave it unloaded.
pub fn install_staged(app_data_dir: &Path) -> bool {
    let dir = app_data_dir.join(PACK_DIR);
    let staging = app_data_dir.join(STAGING_DIR);

    if safe_directory(&dir).is_ok() && dir.join(REMOVE_MARKER).exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    if safe_directory(&dir).is_ok()
        && read_manifest(&staging)
            .is_some_and(|m| matches_engine(&m) && verify_installed(&staging, &m))
    {
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(e) = std::fs::rename(&staging, &dir) {
            log::warn!("CUDA pack: could not install the staged download: {e}");
        }
    }
    matches!(usable_pack(&dir), Err(Unusable::Damaged))
}

enum Unusable {
    Absent,
    Damaged,
}

/// The installed pack's manifest, if it is built for this engine and intact.
fn usable_pack(dir: &Path) -> Result<Manifest, Unusable> {
    let manifest = read_manifest(dir).ok_or(Unusable::Absent)?;
    if !matches_engine(&manifest) {
        log::warn!(
            "CUDA pack built for engine {}, this build runs {}; download it again",
            manifest.engine,
            transcribe_cpp::version()
        );
        return Err(Unusable::Absent);
    }
    if !verify_installed(dir, &manifest) {
        log::warn!("CUDA pack: library integrity verification failed; download it again");
        return Err(Unusable::Damaged);
    }
    Ok(manifest)
}

/// Register the installed pack's CUDA backend, in an engine worker. Must run
/// before the bundled backends: ggml prefers whichever registers first.
pub fn register(app_data_dir: &Path) {
    let dir = app_data_dir.join(PACK_DIR);
    let Ok(manifest) = usable_pack(&dir) else {
        return;
    };

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

/// Delete the pack, or mark it for deletion at the next start when an engine
/// worker has it loaded.
pub fn remove(app_data_dir: &Path) -> std::io::Result<()> {
    for name in [PACK_DIR, STAGING_DIR] {
        safe_directory(&app_data_dir.join(name)).map_err(std::io::Error::other)?;
    }
    let _ = std::fs::remove_dir_all(app_data_dir.join(STAGING_DIR));
    let dir = app_data_dir.join(PACK_DIR);
    if !dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir).or_else(|_| File::create(dir.join(REMOVE_MARKER)).map(drop))
}

/// The pack in the app data dir, and the one download of it allowed at a time.
pub struct CudaPack {
    app_data_dir: PathBuf,
    /// The installed pack failed its integrity check at start (see `activate`).
    damaged: bool,
    cancel: Mutex<Option<CancellationToken>>,
    progress: AtomicU8,
    verifying: AtomicBool,
}

impl CudaPack {
    pub fn new(app_data_dir: PathBuf, damaged: bool) -> Self {
        Self {
            app_data_dir,
            damaged,
            cancel: Mutex::new(None),
            progress: AtomicU8::new(0),
            verifying: AtomicBool::new(false),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<CancellationToken>> {
        lock_recovering(&self.cancel)
    }

    pub fn state(&self) -> PackState {
        match pack_state(&self.app_data_dir) {
            PackState::Installed if self.damaged => PackState::Damaged,
            state => state,
        }
    }

    /// Cancel any download, then delete the pack; one in use goes at the next
    /// start.
    pub fn remove(&self) -> std::io::Result<()> {
        self.cancel();
        remove(&self.app_data_dir)
    }

    /// Progress of the running download, or `None` when idle.
    pub fn progress(&self) -> Option<u8> {
        self.lock()
            .is_some()
            .then(|| self.progress.load(Ordering::Relaxed))
    }

    /// A finished download is being unpacked and checked.
    pub fn verifying(&self) -> bool {
        self.lock().is_some() && self.verifying.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        if let Some(token) = self.lock().as_ref() {
            token.cancel();
        }
    }

    /// Fetch the pack into staging in the background, reported through the
    /// `CudaDownload*` events. A running download is left alone.
    pub fn start_download(app: &AppHandle) {
        // Claimed before returning, so a status read right after reports it.
        let Some(cancel) = app.state::<Self>().claim() else {
            return;
        };
        let _ = CudaDownloadProgress(0).emit(app);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let pack = app.state::<Self>();
            let result = pack.fetch(&app, &cancel).await;
            *pack.lock() = None;
            match result {
                Ok(()) => {
                    let _ = CudaDownloadComplete.emit(&app);
                }
                Err(e) if e == CANCELLED => {
                    let _ = CudaDownloadCancelled.emit(&app);
                }
                Err(e) => {
                    log::warn!("CUDA pack download failed: {e}");
                    let _ = CudaDownloadError(e).emit(&app);
                }
            }
        });
    }

    /// Re-download a pack installed before verification existed.
    pub fn replace_unverified(app: &AppHandle) {
        let app = app.clone();
        // NVML loads the driver library: keep it off the startup path.
        tauri::async_runtime::spawn_blocking(move || {
            if app.state::<Self>().state() == PackState::Unverified && supported() {
                log::info!("CUDA pack predates verification; downloading a verified copy");
                Self::start_download(&app);
            }
        });
    }

    fn claim(&self) -> Option<CancellationToken> {
        let mut slot = self.lock();
        if slot.is_some() {
            return None;
        }
        self.progress.store(0, Ordering::Relaxed);
        self.verifying.store(false, Ordering::Relaxed);
        Some(slot.insert(CancellationToken::new()).clone())
    }

    async fn fetch(&self, app: &AppHandle, cancel: &CancellationToken) -> Result<(), String> {
        let app_data_dir = &self.app_data_dir;
        let platform = PLATFORM.ok_or("CUDA is not available on this platform")?;
        let (manifest, json, signature) = fetch_manifest(platform).await?;
        if !matches_engine(&manifest) {
            return Err("the published CUDA pack does not match this version".to_string());
        }

        let dir = app_data_dir.join(PACK_DIR);
        let staging = app_data_dir.join(STAGING_DIR);
        safe_directory(&dir)?;
        safe_directory(&staging)?;
        std::fs::create_dir_all(&staging).map_err(|e| describe_io(&e))?;
        // Invalidate the completion marker before changing any staged files.
        let _ = std::fs::remove_file(staging.join(MANIFEST));
        let _ = std::fs::remove_file(staging.join(STAMPS));
        let current = read_manifest(&dir).filter(|_| !dir.join(REMOVE_MARKER).exists());

        // Download everything, then unpack and check the pack in one step.
        let total: u64 = manifest.files.iter().map(|f| f.size).sum::<u64>().max(1);
        let mut done = 0;
        let mut downloaded = Vec::new();
        for file in &manifest.files {
            let dest = confined_file(&staging, &file.name)?;
            let unchanged = current.as_ref().is_some_and(|m| {
                m.files
                    .iter()
                    .any(|f| f.name == file.name && f.sha256 == file.sha256)
            });

            // Unchanged files (often the large runtime) are copied, not refetched.
            if unchanged && verified(&confined_file(&dir, &file.name)?, &file.sha256) {
                std::fs::copy(dir.join(&file.name), &dest).map_err(|e| describe_io(&e))?;
            } else if !verified(&dest, &file.sha256) {
                let gz = confined_file(&staging, &format!("{}.gz", file.name))?;
                confined_file(&staging, &format!("{}.part", file.name))?;
                let url = format!("{RELEASES_URL}/{RELEASE_TAG}/{}", file.asset);
                if !archive_complete(&gz, file.size) {
                    download_file(&url, &gz, cancel, file.size, |pct| {
                        let overall = (done + file.size * u64::from(pct) / 100) * 100 / total;
                        self.report(app, overall);
                    })
                    .await?;
                }
                downloaded.push((gz, dest, file.sha256.clone(), file.unpacked_size));
            }
            done += file.size;
            self.report(app, done * 100 / total);
        }

        self.verifying.store(true, Ordering::Relaxed);
        let _ = CudaDownloadVerifying.emit(app);
        let (staged, expected) = (staging.clone(), manifest.clone());
        let cancelled = cancel.clone();
        tokio::task::spawn_blocking(move || {
            for (gz, dest, sha256, size) in downloaded {
                if cancelled.is_cancelled() {
                    return Err(CANCELLED.to_string());
                }
                unpack(&gz, &dest, &sha256, size)?;
            }
            if !files_verified(&staged, &expected, &[]) {
                return Err("CUDA pack integrity check failed".to_string());
            }
            stamp_pack(&staged, &expected).map_err(|e| describe_io(&e))
        })
        .await
        .map_err(|e| e.to_string())??;
        if cancel.is_cancelled() {
            return Err(CANCELLED.to_string());
        }
        std::fs::write(confined_file(&staging, SIGNATURE)?, signature)
            .map_err(|e| describe_io(&e))?;
        std::fs::write(confined_file(&staging, MANIFEST)?, json).map_err(|e| describe_io(&e))
    }

    fn report(&self, app: &AppHandle, pct: u64) {
        let pct = u8::try_from(pct.min(100)).unwrap_or(100);
        if self.progress.swap(pct, Ordering::Relaxed) != pct {
            let _ = CudaDownloadProgress(pct).emit(app);
        }
    }
}

async fn fetch_manifest(platform: &str) -> Result<(Manifest, Vec<u8>, Vec<u8>), String> {
    let url = format!("{RELEASES_URL}/{RELEASE_TAG}/cuda-pack-{platform}.json");
    let json = fetch_small(&url).await?;
    let signature = fetch_small(&format!("{url}.sig")).await.map_err(|e| {
        format!("CUDA pack signature unavailable; only signed packs are supported: {e}")
    })?;
    let manifest = authenticated_manifest(&json, &signature)?;
    Ok((manifest, json, signature))
}

async fn fetch_small(url: &str) -> Result<Vec<u8>, String> {
    use futures_util::StreamExt;
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| describe_request_error(&e))?
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| describe_request_error(&e))?;
    let mut bytes = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|e| describe_request_error(&e))?;
        if bytes.len() + chunk.len() > usize::try_from(MAX_MANIFEST_BYTES).unwrap_or(16_384) {
            return Err("CUDA manifest/signature exceeds size limit".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn authenticated_manifest(json: &[u8], signature: &[u8]) -> Result<Manifest, String> {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../../tauri.conf.json")).map_err(|e| e.to_string())?;
    let key = config["plugins"]["updater"]["pubkey"]
        .as_str()
        .ok_or("updater public key missing")?;
    verify_signature(json, signature, key)?;
    let manifest: Manifest =
        serde_json::from_slice(json).map_err(|e| format!("bad CUDA manifest: {e}"))?;
    validate_manifest(&manifest, PLATFORM.ok_or("CUDA platform unsupported")?)?;
    Ok(manifest)
}

fn verify_signature(json: &[u8], signature: &[u8], key: &str) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    if json.len() > 16_384 || signature.len() > 16_384 {
        return Err("CUDA manifest/signature too large".to_string());
    }
    let decode = |bytes: &[u8]| -> Result<String, String> {
        let decoded = STANDARD
            .decode(bytes.trim_ascii())
            .map_err(|e| e.to_string())?;
        String::from_utf8(decoded).map_err(|e| e.to_string())
    };
    let public_key =
        minisign_verify::PublicKey::decode(&decode(key.as_bytes())?).map_err(|e| e.to_string())?;
    let signature =
        minisign_verify::Signature::decode(&decode(signature)?).map_err(|e| e.to_string())?;
    public_key
        .verify(json, &signature, true)
        .map_err(|e| format!("CUDA manifest signature invalid: {e}"))
}

fn validate_manifest(manifest: &Manifest, platform: &str) -> Result<(), String> {
    let expected = match platform {
        "windows-x86_64" => [
            "cudart64_13.dll",
            "cublasLt64_13.dll",
            "cublas64_13.dll",
            "ggml-cuda.dll",
        ],
        "linux-x86_64" => [
            "libcudart.so.13",
            "libcublasLt.so.13",
            "libcublas.so.13",
            "libggml-cuda.so",
        ],
        _ => return Err("unsupported CUDA platform".to_string()),
    };
    // `release` is not compared: a pack outlives app updates; the engine match decides.
    if manifest.platform != platform || manifest.files.len() != expected.len() {
        return Err("CUDA manifest platform or file set mismatch".to_string());
    }
    for (file, name) in manifest.files.iter().zip(expected) {
        // Exact basenames also reject separators, ADS, prefixes, reserved names,
        // duplicate modules and an unsafe preload order on every platform.
        if file.name != name
            || file.asset != format!("cuda-{platform}-{name}.gz")
            || file.size == 0
            || file.size > MAX_LIBRARY_BYTES
            || file.unpacked_size == 0
            || file.unpacked_size > MAX_LIBRARY_BYTES
            || file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(format!("invalid CUDA library entry: {}", file.name));
        }
    }
    Ok(())
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.is_symlink()
    }
}

fn safe_directory(dir: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(dir) {
        Ok(m) if !m.is_dir() || is_link(&m) => {
            Err(format!("unsafe CUDA directory: {}", dir.display()))
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn confined_file(dir: &Path, name: &str) -> Result<PathBuf, String> {
    safe_directory(dir)?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
        || name.starts_with('.')
        || name.ends_with('.')
        || name.contains("..")
    {
        return Err("unsafe CUDA filename".to_string());
    }
    let path = dir.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(m) if !m.is_file() || is_link(&m) => {
            Err(format!("unsafe CUDA file: {}", path.display()))
        }
        Ok(_) => Ok(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(path),
        Err(e) => Err(e.to_string()),
    }
}

fn read_manifest(dir: &Path) -> Option<Manifest> {
    let read = |name: &str| {
        let path = confined_file(dir, name).ok()?;
        let file = File::open(path).ok()?;
        if file.metadata().ok()?.len() > MAX_MANIFEST_BYTES {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes)
    };
    authenticated_manifest(&read(MANIFEST)?, &read(SIGNATURE)?).ok()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FileStamp {
    name: String,
    size: u64,
    modified_nanos: u128,
}

/// The stamp of every manifest file as it is now; `None` if one is unreadable.
fn current_stamps(dir: &Path, manifest: &Manifest) -> Option<Vec<FileStamp>> {
    manifest
        .files
        .iter()
        .map(|file| {
            let metadata = confined_file(dir, &file.name).ok()?.metadata().ok()?;
            let modified = metadata
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?;
            Some(FileStamp {
                name: file.name.clone(),
                size: metadata.len(),
                modified_nanos: modified.as_nanos(),
            })
        })
        .collect()
}

fn read_stamps(dir: &Path) -> Vec<FileStamp> {
    confined_file(dir, STAMPS)
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_stamps(dir: &Path, stamps: &[FileStamp]) -> std::io::Result<()> {
    let path = confined_file(dir, STAMPS).map_err(std::io::Error::other)?;
    std::fs::write(path, serde_json::to_vec(stamps)?)
}

/// Record the current state of a pack that just passed a full check.
fn stamp_pack(dir: &Path, manifest: &Manifest) -> std::io::Result<()> {
    let stamps = current_stamps(dir, manifest)
        .ok_or_else(|| std::io::Error::other("CUDA pack file unreadable"))?;
    write_stamps(dir, &stamps)
}

/// Check a pack on disk, hashing only the files that changed since their last
/// full check, and refresh the stamps when it passes.
fn verify_installed(dir: &Path, manifest: &Manifest) -> bool {
    let Some(now) = current_stamps(dir, manifest) else {
        return false;
    };
    let known = read_stamps(dir);
    let trusted: Vec<FileStamp> = now
        .iter()
        .filter(|stamp| known.contains(stamp))
        .cloned()
        .collect();
    if !files_verified(dir, manifest, &trusted) {
        return false;
    }
    if known != now {
        if let Err(e) = write_stamps(dir, &now) {
            log::warn!("CUDA pack: could not save its verification stamps: {e}");
        }
    }
    true
}

/// Whether the pack holds exactly the manifest's files, each the right size and
/// hash. Files matching a `trusted` stamp skip the hash.
fn files_verified(dir: &Path, manifest: &Manifest, trusted: &[FileStamp]) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    if !entries.into_iter().all(|entry| {
        entry.is_ok_and(|entry| {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return false;
            };
            matches!(name, MANIFEST | SIGNATURE | STAMPS | REMOVE_MARKER)
                || manifest.files.iter().any(|file| file.name == name)
        })
    }) {
        return false;
    }
    manifest.files.iter().all(|file| {
        confined_file(dir, &file.name).is_ok_and(|path| {
            path.metadata().is_ok_and(|m| m.len() == file.unpacked_size)
                && (trusted.iter().any(|stamp| stamp.name == file.name)
                    || verified(&path, &file.sha256))
        })
    })
}

fn matches_engine(manifest: &Manifest) -> bool {
    manifest.engine == transcribe_cpp::version()
}

/// Gunzip `gz` into `dest`, keeping it only if it matches `sha256`.
/// Fully downloaded by an interrupted run. Unpacking rechecks it and deletes a
/// bad one, so it is refetched.
fn archive_complete(gz: &Path, size: u64) -> bool {
    std::fs::metadata(gz).is_ok_and(|m| m.is_file() && m.len() == size)
}

fn unpack(gz: &Path, dest: &Path, sha256: &str, size: u64) -> Result<(), String> {
    let parent = dest.parent().ok_or("CUDA destination has no parent")?;
    let name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad CUDA filename")?;
    let dest = confined_file(parent, name)?;
    let tmp = confined_file(parent, &format!("{name}.tmp"))?;
    let result = gunzip_hashed(gz, &tmp, size);
    let _ = std::fs::remove_file(gz);

    match result {
        Ok(hash) if hash == sha256 => std::fs::rename(&tmp, dest).map_err(|e| describe_io(&e)),
        Ok(_) => {
            let _ = std::fs::remove_file(&tmp);
            log::warn!("checksum mismatch for {}", dest.display());
            Err("A downloaded CUDA file was damaged. Try again.".to_string())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            log::warn!("unpacking {} failed: {e}", gz.display());
            Err(describe_io(&e))
        }
    }
}

fn gunzip_hashed(gz: &Path, out: &Path, size: u64) -> std::io::Result<String> {
    let reader = flate2::read::GzDecoder::new(BufReader::new(File::open(gz)?));
    let mut writer = BufWriter::new(File::create(out)?);
    let hash = copy_hashed(reader.take(size + 1), &mut writer)?;
    if writer.get_ref().metadata()?.len() > size {
        return Err(std::io::Error::other("CUDA library exceeds size limit"));
    }
    writer.flush()?;
    if writer.get_ref().metadata()?.len() != size {
        return Err(std::io::Error::other("CUDA library size mismatch"));
    }
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
    use std::fmt::Write as _;

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
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        }))
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
#[path = "../../tests/unit/models/cuda.rs"]
mod tests;
