use super::*;

#[test]
fn a_new_recording_makes_every_earlier_ticket_stale() {
    let generation = Generation::default();
    let first = generation.begin();
    assert!(first.is_current());

    let second = generation.begin();

    assert!(!first.is_current());
    assert!(second.is_current());
}

#[test]
fn only_a_current_worker_signals_that_the_stream_is_done() {
    let generation = Generation::default();
    let flag = Arc::new((Mutex::new(false), Condvar::new()));

    let stale = generation.begin();
    let current = generation.begin();
    drop(StreamDone {
        flag: Arc::clone(&flag),
        ticket: stale,
    });
    assert!(!*lock_recovering(&flag.0));

    drop(StreamDone {
        flag: Arc::clone(&flag),
        ticket: current,
    });
    assert!(*lock_recovering(&flag.0));
}
