use super::*;

#[test]
fn a_job_saves_or_cancels_never_both() {
    let saving = Stop::default();
    assert!(saving.commit());
    assert!(
        !saving.cancel(),
        "a cancel once saving has begun comes too late"
    );
    assert!(!saving.cancelled());

    let cancelled = Stop::default();
    assert!(cancelled.cancel());
    assert!(!cancelled.commit(), "a cancelled job must not save");
}
