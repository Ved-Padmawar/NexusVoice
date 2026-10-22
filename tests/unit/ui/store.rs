//! The CUDA offer must open once, where CUDA applies, and never interrupt a
//! download the user already started from About.

use super::*;

fn cuda(supported: bool, pack: PackState, download_progress: Option<u8>) -> Option<CudaStatus> {
    Some(CudaStatus {
        supported,
        pack,
        download_progress,
    })
}

fn absent() -> Option<CudaStatus> {
    cuda(true, PackState::Absent, None)
}

#[test]
fn offers_when_a_model_is_chosen_and_cuda_can_be_added() {
    assert!(offer_due(true, false, false, absent()));
}

#[test]
fn waits_for_the_first_run_model_choice() {
    assert!(!offer_due(false, false, false, absent()));
}

#[test]
fn never_offers_again_once_seen() {
    assert!(!offer_due(true, true, false, absent()));
}

#[test]
fn does_not_stack_a_second_dialog() {
    assert!(!offer_due(true, false, true, absent()));
}

#[test]
fn stays_quiet_until_the_status_is_known() {
    assert!(!offer_due(true, false, false, None));
}

#[test]
fn stays_quiet_where_cuda_cannot_run_or_is_already_present() {
    assert!(!offer_due(
        true,
        false,
        false,
        cuda(false, PackState::Absent, None)
    ));
    for pack in [
        PackState::Installed,
        PackState::PendingInstall,
        PackState::PendingRemoval,
    ] {
        assert!(
            !offer_due(true, false, false, cuda(true, pack, None)),
            "{pack:?}"
        );
    }
}

#[test]
fn does_not_offer_what_is_already_downloading() {
    // Started from About before the delay ended: the dialog would be redundant.
    assert!(!offer_due(
        true,
        false,
        false,
        cuda(true, PackState::Absent, Some(10))
    ));
    assert!(!offer_due(
        true,
        false,
        false,
        cuda(true, PackState::Absent, Some(0))
    ));
}
