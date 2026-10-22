//! The pill state machine. A wrong transition strands the pill (stuck on
//! "processing"), drops a recording, or hides a download behind a stale error.

use super::{start_error, Controller, Input, PillState};
use crate::api::ApiError;
use crate::events::ModelDownload;

fn at(state: PillState) -> Controller {
    Controller {
        pill: Some(state),
        error_generation: 0,
    }
}

#[tokio::test]
async fn a_download_shows_on_an_idle_pill_with_its_progress() {
    let mut c = at(PillState::Idle);
    c.handle(Input::Download(ModelDownload::Queued)).await;
    assert_eq!(c.pill(), &PillState::Downloading(0));
    c.handle(Input::Download(ModelDownload::Progress(42))).await;
    assert_eq!(c.pill(), &PillState::Downloading(42));
    c.handle(Input::Download(ModelDownload::Complete)).await;
    assert_eq!(c.pill(), &PillState::Idle);
}

#[tokio::test]
async fn download_events_never_interrupt_a_recording() {
    for status in [
        ModelDownload::Queued,
        ModelDownload::Progress(10),
        ModelDownload::Complete,
        ModelDownload::Failed("offline".to_string()),
    ] {
        let mut c = at(PillState::Dictation { paused: false });
        c.handle(Input::Download(status)).await;
        assert_eq!(c.pill(), &PillState::Dictation { paused: false });
    }
}

#[tokio::test]
async fn an_error_clears_only_on_its_own_timer() {
    let mut c = at(PillState::Idle);
    c.handle(Input::Failed("first".to_string())).await;
    c.handle(Input::Failed("second".to_string())).await;

    // The first error's timer fires while the second is showing.
    c.handle(Input::ClearError(1)).await;
    assert_eq!(c.pill(), &PillState::Error("second".to_string()));
    c.handle(Input::ClearError(2)).await;
    assert_eq!(c.pill(), &PillState::Idle);
}

#[tokio::test]
async fn a_late_error_timer_does_not_cancel_a_new_recording() {
    let mut c = at(PillState::Idle);
    c.handle(Input::Failed("mic".to_string())).await;
    c.pill = Some(PillState::Recording);
    c.handle(Input::ClearError(1)).await;
    assert_eq!(c.pill(), &PillState::Recording);
}

#[tokio::test]
async fn stray_inputs_outside_their_mode_are_ignored() {
    // A release with no press, a commit or cancel with nothing recording:
    // none may touch the backend or move the pill.
    for input in [Input::PushToTalkReleased, Input::DictationCommit, Input::Cancel] {
        let mut c = at(PillState::Idle);
        c.handle(input).await;
        assert_eq!(c.pill(), &PillState::Idle);
    }
    let mut c = at(PillState::Recording);
    c.handle(Input::DictationToggle).await;
    assert_eq!(c.pill(), &PillState::Recording, "dictation cannot start mid push-to-talk");
}

#[test]
fn microphone_failures_get_a_plain_message() {
    let message = |raw: &str| start_error(&ApiError::new("capture_failed", raw));
    assert_eq!(message("No input device available"), "No microphone found");
    assert_eq!(message("Permission denied by the system"), "Mic access denied");
    assert_eq!(message("device busy"), "device busy");
}
