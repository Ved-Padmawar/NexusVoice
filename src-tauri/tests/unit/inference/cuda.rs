use super::*;

fn install(dir: &Path, engine: &str) {
    let manifest = Manifest {
        engine: engine.to_string(),
        files: vec![PackFile {
            name: "ggml-cuda.dll".to_string(),
            asset: "cuda-windows-x86_64-ggml-cuda.dll.gz".to_string(),
            sha256: "00".to_string(),
            size: 1,
        }],
    };
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
}

fn gzip(path: &Path, bytes: &[u8]) {
    use flate2::{write::GzEncoder, Compression};

    let mut encoder = GzEncoder::new(File::create(path).unwrap(), Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap();
}

#[test]
fn a_pack_is_installed_only_for_this_engine() {
    let data = tempfile::tempdir().unwrap();
    assert_eq!(pack_state(data.path()), PackState::Absent);

    install(&data.path().join(PACK_DIR), &transcribe_cpp::version());
    assert_eq!(pack_state(data.path()), PackState::Installed);
}

#[test]
fn a_pack_for_another_engine_is_never_loaded() {
    // Its module would not match this build's ggml.
    let data = tempfile::tempdir().unwrap();
    install(&data.path().join(PACK_DIR), "0.0.0-other");
    assert_eq!(pack_state(data.path()), PackState::Absent);
}

#[test]
fn a_staged_download_replaces_the_pack_at_activation() {
    let data = tempfile::tempdir().unwrap();
    install(&data.path().join(PACK_DIR), "0.0.0-old");
    install(&data.path().join(STAGING_DIR), &transcribe_cpp::version());
    assert_eq!(pack_state(data.path()), PackState::PendingInstall);

    activate(data.path());

    assert!(!data.path().join(STAGING_DIR).exists());
    assert_eq!(pack_state(data.path()), PackState::Installed);
}

#[test]
fn a_pack_marked_for_removal_is_deleted_at_activation() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join(PACK_DIR);
    install(&dir, &transcribe_cpp::version());
    std::fs::write(dir.join(REMOVE_MARKER), b"").unwrap();
    assert_eq!(pack_state(data.path()), PackState::PendingRemoval);

    activate(data.path());

    assert!(!dir.exists());
    assert_eq!(pack_state(data.path()), PackState::Absent);
}

#[test]
fn remove_deletes_an_unloaded_pack_and_any_staged_download() {
    let data = tempfile::tempdir().unwrap();
    install(&data.path().join(PACK_DIR), &transcribe_cpp::version());
    install(&data.path().join(STAGING_DIR), &transcribe_cpp::version());

    remove(data.path()).unwrap();

    assert_eq!(pack_state(data.path()), PackState::Absent);
    assert!(!data.path().join(PACK_DIR).exists());
}

#[test]
fn only_gpus_the_pack_has_kernels_for_qualify() {
    assert!(
        meets_requirements((7, 5), MIN_DRIVER_CUDA),
        "RTX 20 / GTX 16"
    );
    assert!(meets_requirements((12, 0), MIN_DRIVER_CUDA), "RTX 50");
    assert!(
        !meets_requirements((6, 1), MIN_DRIVER_CUDA),
        "GTX 10 has no kernels in the pack"
    );
    assert!(
        !meets_requirements((7, 0), MIN_DRIVER_CUDA),
        "Volta predates sm_75"
    );
}

#[test]
fn a_driver_older_than_cuda_13_never_qualifies() {
    // CUDA 12.9 drivers (12090) cannot load the CUDA 13 runtime.
    assert!(!meets_requirements((8, 6), 12_090));
    assert!(meets_requirements((8, 6), 13_020));
}

#[test]
fn an_idle_download_reports_no_progress_and_ignores_cancel() {
    // The UI reads `None` as "no download running".
    let download = CudaDownload::default();
    download.cancel();
    assert_eq!(download.progress(), None);
}

#[test]
fn unpack_publishes_only_a_file_matching_its_checksum() {
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("lib.dll.gz");
    let dest = dir.path().join("lib.dll");
    let sha = copy_hashed(&b"cuda"[..], std::io::sink()).unwrap();

    gzip(&gz, b"cuda");
    assert!(unpack(&gz, &dest, "bad").is_err());
    assert!(!dest.exists(), "a corrupt file must never be published");

    gzip(&gz, b"cuda");
    unpack(&gz, &dest, &sha).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"cuda");
    assert!(verified(&dest, &sha));
    assert!(!gz.exists(), "the download is dropped once unpacked");
}
