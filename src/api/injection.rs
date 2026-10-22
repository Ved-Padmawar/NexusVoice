//! Text injection status.

pub use super::dto::InjectionStatus;

pub async fn get_injection_status() -> InjectionStatus {
    InjectionStatus::detect().await
}
