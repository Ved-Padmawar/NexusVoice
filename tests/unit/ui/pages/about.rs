//! The About tiles: each store state must map to the right words and the one
//! action that moves it forward.

use super::*;
use crate::api::CudaStatus;

fn update(status: UpdateStatus, version: Option<&str>) -> UpdateState {
    let mut update = UpdateState::default();
    update.status = status;
    update.version = version.map(Into::into);
    update
}

fn cuda(pack: PackState, progress: Option<u8>) -> CudaStatus {
    CudaStatus {
        supported: true,
        pack,
        download_progress: progress,
    }
}

fn detail(tile: &Tile) -> (&str, Option<Tone>) {
    match &tile.body {
        Body::Detail { text, tone } => (text, *tone),
        Body::Progress { .. } => panic!("expected a detail line"),
    }
}

#[test]
fn an_available_update_names_its_version_and_offers_the_download() {
    let tile = update_tile(&update(UpdateStatus::Available, Some("1.17.0")), "1.16.0");
    assert_eq!(tile.title, "v1.17.0 available");
    assert_eq!(tile.action, Some(Action::InstallUpdate));
}

#[test]
fn a_downloading_update_shows_progress_and_no_button_to_double_start_it() {
    let mut state = update(UpdateStatus::Downloading, Some("1.17.0"));
    state.progress = 42;
    let tile = update_tile(&state, "1.16.0");
    assert_eq!(tile.title, "Downloading v1.17.0");
    assert_eq!(
        tile.body,
        Body::Progress {
            percent: 42,
            cancel: false
        }
    );
    assert_eq!(tile.action, None);
}

#[test]
fn an_installed_update_asks_for_a_restart() {
    let tile = update_tile(&update(UpdateStatus::Ready, Some("1.17.0")), "1.16.0");
    assert_eq!(detail(&tile).0, "Restart to apply v1.17.0");
    assert_eq!(tile.action, Some(Action::Restart));
}

#[test]
fn a_failed_check_shows_the_error_as_the_title_and_retries_the_check() {
    let mut state = update(UpdateStatus::Failed, None);
    state.error = Some("network unreachable".into());
    let tile = update_tile(&state, "1.16.0");
    assert_eq!(tile.title, "network unreachable");
    assert_eq!(tile.tone, Tone::Danger);
    assert_eq!(tile.action, Some(Action::RetryCheck));

    state.error = None;
    assert_eq!(update_tile(&state, "1.16.0").title, "Update failed");
}

#[test]
fn checking_spins_and_cannot_be_clicked_again() {
    let tile = update_tile(&update(UpdateStatus::Checking, None), "1.16.0");
    assert!(tile.spinning);
    assert_eq!(tile.action, Some(Action::Checking));
}

#[test]
fn up_to_date_quotes_the_running_version() {
    let tile = update_tile(&update(UpdateStatus::UpToDate, None), "1.16.0");
    assert_eq!(detail(&tile).0, "v1.16.0 is the latest");
}

#[test]
fn cuda_is_hidden_where_it_is_unsupported_or_not_yet_known() {
    let unsupported = Some(CudaStatus {
        supported: false,
        pack: PackState::Absent,
        download_progress: None,
    });
    assert_eq!(cuda_tile(unsupported, None, None), None);
    assert_eq!(cuda_tile(None, None, None), None);
}

#[test]
fn a_running_cuda_download_can_be_cancelled() {
    let tile = cuda_tile(Some(cuda(PackState::Absent, Some(30))), None, None).expect("tile");
    assert_eq!(
        tile.body,
        Body::Progress {
            percent: 30,
            cancel: true
        }
    );
}

#[test]
fn pending_cuda_changes_need_a_restart_to_apply() {
    let installed =
        cuda_tile(Some(cuda(PackState::PendingInstall, None)), None, None).expect("tile");
    assert_eq!(installed.action, Some(Action::Restart));
    let removed = cuda_tile(Some(cuda(PackState::PendingRemoval, None)), None, None).expect("tile");
    assert_eq!(removed.title, "CUDA removed");
    assert_eq!(removed.action, Some(Action::Restart));
}

#[test]
fn installed_cuda_is_active_only_when_the_backend_is_cuda() {
    let active =
        cuda_tile(Some(cuda(PackState::Installed, None)), Some("cuda"), None).expect("tile");
    assert_eq!(
        (active.title.as_str(), active.tone),
        ("CUDA active", Tone::Success)
    );
    let idle =
        cuda_tile(Some(cuda(PackState::Installed, None)), Some("vulkan"), None).expect("tile");
    assert_eq!(idle.title, "CUDA installed, not in use");
    assert_eq!(idle.tone, Tone::Accent);
    assert_eq!(idle.action, Some(Action::RemoveCuda));
}

#[test]
fn a_cuda_error_replaces_the_hint_but_never_a_restart_prompt() {
    let absent = cuda_tile(
        Some(cuda(PackState::Absent, None)),
        None,
        Some("checksum mismatch"),
    )
    .expect("tile");
    assert_eq!(detail(&absent), ("checksum mismatch", Some(Tone::Danger)));
    assert_eq!(absent.action, Some(Action::DownloadCuda));

    let pending = cuda_tile(
        Some(cuda(PackState::PendingInstall, None)),
        None,
        Some("stale"),
    )
    .expect("tile");
    assert_eq!(detail(&pending).0, "Restart to start using it");
}
