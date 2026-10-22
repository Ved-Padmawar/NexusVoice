//! Text injection status. The platform strategies live in `crate::injection`.

pub use super::dto::InjectionStatus;

/// Which injection route this session uses, for the settings page.
pub async fn get_injection_status() -> InjectionStatus {
    InjectionStatus::detect().await
}
