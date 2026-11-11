use super::*;

#[test]
fn pct_of_reports_zero_when_total_is_unknown() {
    // Server sent no Content-Length; reporting 0 beats dividing by zero.
    assert_eq!(pct_of(1234, 0), 0);
}

#[test]
fn pct_of_scales_and_saturates() {
    assert_eq!(pct_of(0, 100), 0);
    assert_eq!(pct_of(50, 100), 50);
    assert_eq!(pct_of(100, 100), 100);
    // A resumed transfer can overshoot a stale total; never exceed 100.
    assert_eq!(pct_of(150, 100), 100);
}

#[test]
fn clean_stale_parts_removes_only_parts_no_catalog_model_resumes() {
    // A private tempdir, not a shared fixed path: two concurrent runs of this
    // suite must not sweep each other's fixture out from under them.
    let dir = tempfile::tempdir().expect("tempdir");
    let part = dir.path().join("ggml-tiny.en-q5_1.part");
    let model = dir.path().join("ggml-tiny.en-q5_1.gguf");
    let resumable = dir
        .path()
        .join(Path::new(&catalog::all()[0].filename).with_extension("part"));
    std::fs::write(&resumable, b"half a model").unwrap();
    std::fs::write(&part, b"partial").unwrap();
    std::fs::write(&model, b"complete").unwrap();

    clean_stale_parts(dir.path());

    assert!(!part.exists(), "stale .part should be swept");
    assert!(
        resumable.exists(),
        "a catalog model's .part resumes after a restart"
    );
    assert!(model.exists(), "finished model must survive");
}

#[test]
fn clean_stale_parts_tolerates_a_missing_directory() {
    // Startup runs this before the models dir is guaranteed to exist.
    clean_stale_parts(&std::env::temp_dir().join("nv_no_such_dir"));
}

#[tokio::test]
async fn a_stream_without_content_length_cannot_write_past_the_limit() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0);
            request.extend_from_slice(&buffer[..read]);
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\ngo\r\n3\r\nod!\r\n0\r\n\r\n").unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("model.part");
    let error = transfer(
        &format!("http://{address}/model"),
        &part,
        &CancellationToken::new(),
        4,
        &mut |_| {},
    )
    .await
    .unwrap_err();
    server.join().unwrap();
    assert_eq!(error, TOO_LARGE);
    assert!(part.metadata().unwrap().len() <= 4);
}

#[tokio::test]
async fn a_cancel_does_not_wait_for_a_stalled_server_to_answer() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    // Accepts, then never sends a status line: `send` would sit out READ_TIMEOUT.
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        canceller.cancel();
    });

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        transfer(
            &format!("http://{address}/model"),
            &dir.path().join("model.part"),
            &cancel,
            4,
            &mut |_| {},
        ),
    )
    .await;
    server.abort();
    assert_eq!(
        result.expect("cancel must not wait out the read timeout"),
        Err(CANCELLED.to_string())
    );
}

#[test]
fn reads_the_first_byte_of_a_content_range() {
    assert_eq!(parse_range_start("bytes 100-199/200"), Some(100));
    assert_eq!(parse_range_start("bytes 0-9/10"), Some(0));
}

#[test]
fn a_malformed_content_range_has_no_start() {
    for header in ["", "100-199/200", "bytes */200", "bytes abc-199/200"] {
        assert_eq!(parse_range_start(header), None, "{header:?}");
    }
}

#[test]
fn the_startup_sweep_keeps_out_while_a_download_holds_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("model.part");
    std::fs::write(&part, b"in flight").unwrap();

    let lock = DownloadLock::acquire(dir.path(), "model").unwrap();
    clean_stale_parts(dir.path());
    assert!(
        part.exists(),
        "another process's active download must survive"
    );

    drop(lock);
    clean_stale_parts(dir.path());
    assert!(!part.exists(), "debris goes once nothing is downloading");
    assert!(!dir.path().join("model.lock").exists());
}

#[test]
fn different_models_can_download_together() {
    let dir = tempfile::tempdir().unwrap();
    let first = DownloadLock::acquire(dir.path(), "a.gguf").unwrap();
    assert!(DownloadLock::acquire(dir.path(), "b.gguf").is_ok());
    drop(first);
}

#[test]
fn the_same_model_cannot_download_twice() {
    let dir = tempfile::tempdir().unwrap();
    let first = DownloadLock::acquire(dir.path(), "a.gguf").unwrap();
    assert!(DownloadLock::acquire(dir.path(), "a.gguf").is_err());
    drop(first);
    assert!(DownloadLock::acquire(dir.path(), "a.gguf").is_ok());
}

#[test]
fn a_download_that_cannot_fit_fails_before_transferring() {
    let dir = tempfile::tempdir().unwrap();
    let error = ensure_free_space(&dir.path().join("model.gguf"), u64::MAX).unwrap_err();
    assert!(
        error.starts_with("Not enough disk space: needs "),
        "{error}"
    );
}

#[test]
fn a_sibling_folder_sharing_a_mount_prefix_is_not_on_that_mount() {
    let models = Path::new("/home-backup/models");
    assert!(!is_on_mount(models, Path::new("/home")));
    assert!(is_on_mount(models, Path::new("/home-backup")));
    assert!(is_on_mount(models, Path::new("/")));
}
