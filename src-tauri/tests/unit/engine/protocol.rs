use super::*;

fn run_request() -> Request {
    Request::Run {
        options: RunOptions::default(),
    }
}

#[test]
fn a_message_and_its_audio_round_trip() {
    let mut buf = Vec::new();
    let pcm = [0.0f32, -1.0, 0.5, f32::MIN_POSITIVE];
    write_message(&mut buf, &run_request(), Some(&pcm)).unwrap();

    let (request, got): (Request, Vec<f32>) = read_message(&mut buf.as_slice()).unwrap().unwrap();

    assert!(matches!(request, Request::Run { .. }));
    assert_eq!(got, pcm);
}

#[test]
fn only_an_eof_between_frames_is_a_clean_end() {
    let empty: &[u8] = &[];
    assert!(read_message::<Request>(&mut &*empty).unwrap().is_none());

    let mut buf = Vec::new();
    write_message(&mut buf, &run_request(), Some(&[0.5f32; 8])).unwrap();
    // Cut inside the length prefix, the JSON, the PCM length and the PCM.
    for cut in [1, 3, 6, buf.len() - 34, buf.len() - 1] {
        let partial = &buf[..cut];
        let err = read_message::<Request>(&mut &*partial).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof, "cut at {cut}");
    }
}
