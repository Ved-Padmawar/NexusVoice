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
fn clean_stale_parts_removes_only_part_files() {
    // A private tempdir, not a shared fixed path: two concurrent runs of this
    // suite must not sweep each other's fixture out from under them.
    let dir = tempfile::tempdir().expect("tempdir");
    let part = dir.path().join("ggml-tiny.en-q5_1.part");
    let model = dir.path().join("ggml-tiny.en-q5_1.gguf");
    std::fs::write(&part, b"partial").unwrap();
    std::fs::write(&model, b"complete").unwrap();

    clean_stale_parts(dir.path());

    assert!(!part.exists(), "stale .part should be swept");
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
    assert!(error.contains("exceeds expected size"));
    assert!(part.metadata().unwrap().len() <= 4);
}
