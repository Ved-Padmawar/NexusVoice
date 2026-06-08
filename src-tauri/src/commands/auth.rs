//! Authentication & session commands.
//!
//! Local-profile model (no tokens): register/login persist the signed-in user in
//! the `app_session` table and mirror the id in `AppState`; logout clears both.

use tauri::State;

use crate::database::models::user::User;
use crate::state::AppState;

use super::dto::{AuthStateResponse, UserResponse};
use super::error::ApiError;

#[tauri::command]
pub async fn get_auth_state(state: State<'_, AppState>) -> Result<AuthStateResponse, ApiError> {
    let user_id = state.current_user_id().await;
    Ok(AuthStateResponse {
        authenticated: user_id.is_some(),
        user_id,
    })
}

#[tauri::command]
pub async fn get_current_user(state: State<'_, AppState>) -> Result<Option<User>, ApiError> {
    let Some(user_id) = state.current_user_id().await else {
        return Ok(None);
    };
    let repo = crate::database::repositories::user::UserRepository::new(state.db().await.clone());
    let user = repo.get_by_id(user_id).await?;
    Ok(user)
}

#[tauri::command]
pub async fn register(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<UserResponse, ApiError> {
    let user = state.auth().await.register(&email, &password).await?;
    state.set_auth_session(user.id).await;
    Ok(user.into())
}

#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<UserResponse, ApiError> {
    let user = state.auth().await.login(&email, &password).await?;
    state.set_auth_session(user.id).await;
    Ok(user.into())
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.auth().await.logout().await?;
    state.clear_auth_session().await;
    Ok(())
}
