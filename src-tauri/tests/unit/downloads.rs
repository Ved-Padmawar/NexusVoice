use super::*;

#[test]
fn enqueue_registers_a_queued_entry() {
    let d = Downloads::new();
    assert!(d.enqueue("whisper-tiny").is_some());
    assert!(d.is_pending("whisper-tiny"));

    let snap = d.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].0, "whisper-tiny");
    assert_eq!(snap[0].1, "queued");
    assert_eq!(snap[0].2, 0);
}

#[test]
fn enqueue_is_idempotent_while_pending() {
    // A second press on the same card must not start a second transfer.
    let d = Downloads::new();
    assert!(d.enqueue("whisper-tiny").is_some());
    assert!(d.enqueue("whisper-tiny").is_none());

    d.set_running("whisper-tiny");
    assert!(d.enqueue("whisper-tiny").is_none());
}

#[test]
fn a_failed_download_can_be_retried() {
    let d = Downloads::new();
    d.enqueue("whisper-tiny");
    d.set_error("whisper-tiny", "network unreachable".into());

    // An error is terminal for the run but not for the model.
    assert!(!d.is_pending("whisper-tiny"));
    assert!(d.enqueue("whisper-tiny").is_some());
    assert_eq!(d.snapshot()[0].1, "queued");
}

#[test]
fn progress_and_error_surface_in_the_snapshot() {
    let d = Downloads::new();
    d.enqueue("parakeet");
    d.set_running("parakeet");
    d.set_progress("parakeet", 42);

    let snap = d.snapshot();
    assert_eq!(snap[0].1, "running");
    assert_eq!(snap[0].2, 42);
    assert_eq!(snap[0].3, None);

    d.set_error("parakeet", "boom".into());
    let snap = d.snapshot();
    assert_eq!(snap[0].1, "error");
    assert_eq!(snap[0].3.as_deref(), Some("boom"));
}

#[test]
fn cancel_trips_the_token_for_a_pending_download() {
    let d = Downloads::new();
    let token = d.enqueue("canary").expect("first enqueue registers");
    assert!(!token.is_cancelled());

    assert!(d.cancel("canary"));
    assert!(token.is_cancelled());
}

#[test]
fn cancel_reports_false_when_there_is_nothing_to_stop() {
    let d = Downloads::new();
    assert!(!d.cancel("never-queued"));

    // An entry left holding an error is not cancellable either.
    d.enqueue("moonshine");
    d.set_error("moonshine", "failed".into());
    assert!(!d.cancel("moonshine"));
}

#[test]
fn remove_clears_the_entry() {
    let d = Downloads::new();
    d.enqueue("nemotron");
    d.remove("nemotron");

    assert!(!d.is_pending("nemotron"));
    assert!(d.snapshot().is_empty());
}

#[test]
fn downloads_are_tracked_independently() {
    let d = Downloads::new();
    d.enqueue("a");
    d.enqueue("b");
    d.set_running("a");
    d.set_progress("a", 70);

    // Cancelling one leaves the other untouched.
    assert!(d.cancel("a"));
    d.remove("a");

    let snap = d.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].0, "b");
    assert_eq!(snap[0].1, "queued");
}
