//! Authentication & session commands.

use tauri::State;

use crate::database::models::user::User;
use crate::state::AppState;

use super::dto::{AuthResponse, AuthStateResponse, TokenPairResponse, UserResponse};
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
pub async fn store_refresh_token(
    state: State<'_, AppState>,
    refresh_token: String,
    user_id: i64,
    access_token: String,
) -> Result<(), ApiError> {
    state
        .save_refresh_token(&refresh_token)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
    state.set_auth_session(user_id, access_token).await;
    Ok(())
}

#[tauri::command]
pub async fn clear_stored_token(
    state: State<'_, AppState>,
    refresh_token: Option<String>,
) -> Result<(), ApiError> {
    if let Some(token) = refresh_token {
        let _ = state.auth().await.revoke_token(&token).await;
    }
    state.delete_refresh_token();
    state.clear_auth_session().await;
    Ok(())
}

#[tauri::command]
pub async fn register(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<UserResponse, ApiError> {
    let user = state.auth().await.register(&email, &password).await?;
    Ok(user.into())
}

#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<UserResponse, ApiError> {
    let user = state.auth().await.login(&email, &password).await?;
    Ok(user.into())
}

#[tauri::command]
pub async fn login_with_tokens(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<AuthResponse, ApiError> {
    let (user, pair) = state
        .auth()
        .await
        .login_with_tokens(&email, &password)
        .await?;
    Ok(AuthResponse {
        user: user.into(),
        tokens: pair.into(),
    })
}

#[tauri::command]
pub async fn register_with_tokens(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<AuthResponse, ApiError> {
    let (user, pair) = state
        .auth()
        .await
        .register_with_tokens(&email, &password)
        .await?;
    Ok(AuthResponse {
        user: user.into(),
        tokens: pair.into(),
    })
}

#[tauri::command]
pub async fn refresh_token(
    state: State<'_, AppState>,
    refresh_token: String,
) -> Result<TokenPairResponse, ApiError> {
    let pair = state.auth().await.refresh_tokens(&refresh_token).await?;
    Ok(pair.into())
}

#[tauri::command]
pub async fn logout_token(
    state: State<'_, AppState>,
    refresh_token: String,
) -> Result<(), ApiError> {
    state.auth().await.revoke_token(&refresh_token).await?;
    Ok(())
}
