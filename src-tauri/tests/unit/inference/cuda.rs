use super::*;

fn install(dir: &Path, engine: &str) {
    let manifest = Manifest {
        platform: PLATFORM.unwrap().to_string(),
        release: RELEASE_TAG.to_string(),
        engine: engine.to_string(),
        files: vec![PackFile {
            name: "ggml-cuda.dll".to_string(),
            asset: "cuda-windows-x86_64-ggml-cuda.dll.gz".to_string(),
            sha256: "00".to_string(),
            size: 1,
            unpacked_size: 4,
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
fn an_unsigned_pack_is_unverified_never_installed() {
    let data = tempfile::tempdir().unwrap();
    assert_eq!(pack_state(data.path()), PackState::Absent);

    install(&data.path().join(PACK_DIR), &transcribe_cpp::version());
    assert_eq!(pack_state(data.path()), PackState::Unverified);
}

#[test]
fn a_pack_folder_without_a_manifest_is_absent() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join(PACK_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ggml-cuda.dll"), b"partial").unwrap();

    assert_eq!(pack_state(data.path()), PackState::Absent);
}

#[test]
fn a_fully_downloaded_archive_is_not_fetched_again() {
    let data = tempfile::tempdir().unwrap();
    let gz = data.path().join("ggml-cuda.dll.gz");
    std::fs::write(&gz, b"complete").unwrap();

    assert!(archive_complete(&gz, 8));
}

#[test]
fn a_missing_or_partial_archive_is_fetched() {
    let data = tempfile::tempdir().unwrap();
    let gz = data.path().join("ggml-cuda.dll.gz");
    assert!(!archive_complete(&gz, 8));

    std::fs::write(&gz, b"part").unwrap();
    assert!(!archive_complete(&gz, 8));
}

#[test]
fn only_one_download_can_claim_the_pack() {
    let data = tempfile::tempdir().unwrap();
    let pack = CudaPack::new(data.path().to_path_buf());

    assert!(pack.claim().is_some());
    assert!(pack.claim().is_none());
    assert_eq!(pack.progress(), Some(0));
    assert!(!pack.verifying());
}

#[test]
fn the_engine_version_must_match() {
    let mut manifest = valid_manifest(PLATFORM.unwrap());
    manifest.engine = transcribe_cpp::version();
    assert!(matches_engine(&manifest));
    manifest.engine = "0.0.0-other".to_string();
    assert!(!matches_engine(&manifest));
}

#[test]
fn an_unsigned_staged_pack_cannot_replace_an_existing_pack() {
    let data = tempfile::tempdir().unwrap();
    install(&data.path().join(PACK_DIR), "0.0.0-old");
    install(&data.path().join(STAGING_DIR), &transcribe_cpp::version());
    assert_eq!(pack_state(data.path()), PackState::Unverified);

    activate(data.path());

    assert!(data.path().join(STAGING_DIR).exists());
    let retained: Manifest =
        serde_json::from_slice(&std::fs::read(data.path().join(PACK_DIR).join(MANIFEST)).unwrap())
            .unwrap();
    assert_eq!(retained.engine, "0.0.0-old");
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
    let download = CudaPack::new(std::env::temp_dir());
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
    assert!(unpack(&gz, &dest, "bad", 4).is_err());
    assert!(!dest.exists(), "a corrupt file must never be published");

    gzip(&gz, b"cuda");
    unpack(&gz, &dest, &sha, 4).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"cuda");
    assert!(verified(&dest, &sha));
    assert!(!gz.exists(), "the download is dropped once unpacked");
}

#[test]
fn authenticates_exact_manifest_bytes_with_tauri_signatures() {
    let json = include_bytes!("../../fixtures/cuda-signing/manifest.json");
    let signature = include_bytes!("../../fixtures/cuda-signing/manifest.json.sig");
    let key = include_str!("../../fixtures/cuda-signing/public-key.txt").trim();
    verify_signature(json, signature, key).unwrap();
    let mut tampered = json.to_vec();
    tampered[0] ^= 1;
    assert!(verify_signature(&tampered, signature, key).is_err());
    assert!(verify_signature(json, b"", key).is_err());
    assert!(verify_signature(json, b"Zm9yZ2Vk", key).is_err());
    // This fixture key is deliberately different from the application trust.
    assert!(authenticated_manifest(json, signature).is_err());
}

fn valid_manifest(platform: &str) -> Manifest {
    let mut manifest: Manifest =
        serde_json::from_slice(include_bytes!("../../fixtures/cuda-signing/manifest.json"))
            .unwrap();
    manifest.release = RELEASE_TAG.to_string();
    manifest.platform = platform.to_string();
    let names = if platform.starts_with("linux") {
        [
            "libcudart.so.13",
            "libcublasLt.so.13",
            "libcublas.so.13",
            "libggml-cuda.so",
        ]
    } else {
        [
            "cudart64_13.dll",
            "cublasLt64_13.dll",
            "cublas64_13.dll",
            "ggml-cuda.dll",
        ]
    };
    for (file, name) in manifest.files.iter_mut().zip(names) {
        file.name = name.to_string();
        file.asset = format!("cuda-{platform}-{name}.gz");
    }
    manifest
}

#[test]
fn rejects_paths_prefixes_wrong_assets_and_load_order_before_io() {
    for platform in ["windows-x86_64", "linux-x86_64"] {
        let manifest = valid_manifest(platform);
        validate_manifest(&manifest, platform).unwrap();
        for name in [
            "../outside.txt",
            "..\\outside.dll",
            "/outside.dll",
            "C:evil.dll",
            "C:\\evil.dll",
            "CON.dll",
            "cudart64_13.dll:stream",
            "\\\\?\\C:\\evil.dll",
            "cudart64_13.dll.",
        ] {
            let mut bad = manifest.clone();
            bad.files[0].name = name.to_string();
            assert!(validate_manifest(&bad, platform).is_err(), "{name}");
        }
        let mut bad = manifest.clone();
        bad.files.swap(0, 3);
        assert!(validate_manifest(&bad, platform).is_err());
        let mut bad = manifest.clone();
        bad.files[0].asset = "../other.gz".to_string();
        assert!(validate_manifest(&bad, platform).is_err());
    }
}

#[test]
fn unpack_is_bounded_and_rejects_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("lib.dll.gz");
    let dest = dir.path().join("lib.dll");
    gzip(&gz, &[0; 1024]);
    assert!(unpack(&gz, &dest, "00", 4).is_err());
    assert!(!dest.exists());
    assert!(!dest.with_file_name("lib.dll.tmp").exists());
    assert!(confined_file(dir.path(), "../outside").is_err());
}

#[test]
fn unsigned_extra_modules_are_rejected_even_beside_valid_libraries() {
    let dir = tempfile::tempdir().unwrap();
    let mut manifest = valid_manifest(PLATFORM.unwrap());
    for file in &mut manifest.files {
        std::fs::write(dir.path().join(&file.name), b"cuda").unwrap();
        file.unpacked_size = 4;
        file.sha256 = copy_hashed(&b"cuda"[..], std::io::sink()).unwrap();
    }
    assert!(files_verified(dir.path(), &manifest));
    std::fs::write(dir.path().join("ggml-unsigned.dll"), b"evil").unwrap();
    assert!(!files_verified(dir.path(), &manifest));
}

#[test]
fn linked_pack_directories_cannot_escape_the_staging_root() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("outside");
    let link = dir.path().join("cuda.next");
    std::fs::create_dir(&target).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let rejected = safe_directory(&link).is_err() && confined_file(&link, MANIFEST).is_err();
    // Remove the link itself before the temporary directory is cleaned up.
    #[cfg(windows)]
    std::fs::remove_dir(&link).unwrap();
    #[cfg(unix)]
    std::fs::remove_file(&link).unwrap();
    assert!(rejected);
    assert!(target.is_dir());
}
