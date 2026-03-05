use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::audio::{AudioInput, AudioPipeline, AudioPipelineConfig};
use crate::auth::{AuthError, TokenPair};
use crate::database::dto::{dictionary::CreateDictionaryEntry, transcript::CreateTranscript};
use crate::database::models::{dictionary::DictionaryEntry, transcript::Transcript, user::User};
use crate::database::repositories::{
    dictionary::DictionaryRepository, transcript::TranscriptRepository,
};
use crate::hardware::{detect_profile, SysinfoProvider};
use crate::inference::{ExecutionProvider, MockInferenceEngine};
use crate::models::{select_model, ModelSize};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ApiError {
    code: String,
    message: String,
}

impl ApiError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        Self::new("database_error", map_db_error(&value))
    }
}

impl From<AuthError> for ApiError {
    fn from(value: AuthError) -> Self {
        match value {
            AuthError::EmailTaken => Self::new("email_taken", "email already registered"),
            AuthError::InvalidCredentials => {
                Self::new("invalid_credentials", "invalid credentials")
            }
            AuthError::PasswordHash => Self::new("password_hash_failed", "password hashing failed"),
            AuthError::TokenGeneration => Self::new("token_error", "token generation failed"),
            AuthError::TokenExpired => Self::new("token_expired", "token expired"),
            AuthError::TokenInvalid => Self::new("token_invalid", "token invalid"),
            AuthError::TokenRevoked => Self::new("token_revoked", "token revoked or not found"),
            AuthError::Database(err) => Self::new("database_error", map_db_error(&err)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub created_at: String,
}

impl From<User> for UserResponse {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResponse {
    pub id: i64,
    pub content: String,
    pub created_at: String,
}

impl From<Transcript> for TranscriptResponse {
    fn from(value: Transcript) -> Self {
        Self {
            id: value.id,
            content: value.content,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryResponse {
    pub id: i64,
    pub term: String,
    pub replacement: String,
    pub created_at: String,
}

impl From<DictionaryEntry> for DictionaryResponse {
    fn from(value: DictionaryEntry) -> Self {
        Self {
            id: value.id,
            term: value.term,
            replacement: value.replacement,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPairResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in_seconds: i64,
}

impl From<TokenPair> for TokenPairResponse {
    fn from(p: TokenPair) -> Self {
        Self {
            access_token: p.access_token,
            refresh_token: p.refresh_token,
            expires_in_seconds: p.expires_in_seconds,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub user: UserResponse,
    pub tokens: TokenPairResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileResponse {
    pub gpu_type: String,
    pub vram_gb: f32,
    pub execution_provider: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoResponse {
    pub size: String,
    pub reason: String,
    pub execution_provider: String,
}

#[tauri::command]
pub async fn register(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<UserResponse, ApiError> {
    let user = state.auth.register(&email, &password).await?;
    Ok(user.into())
}

#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<UserResponse, ApiError> {
    let user = state.auth.login(&email, &password).await?;
    Ok(user.into())
}

#[tauri::command]
pub async fn login_with_tokens(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<AuthResponse, ApiError> {
    let (user, pair) = state.auth.login_with_tokens(&email, &password).await?;
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
    let (user, pair) = state.auth.register_with_tokens(&email, &password).await?;
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
    let pair = state.auth.refresh_tokens(&refresh_token).await?;
    Ok(pair.into())
}

#[tauri::command]
pub async fn logout_token(
    state: State<'_, AppState>,
    refresh_token: String,
) -> Result<(), ApiError> {
    state.auth.revoke_token(&refresh_token).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_hardware_profile() -> Result<HardwareProfileResponse, ApiError> {
    let provider = SysinfoProvider::new();
    let profile = detect_profile(&provider);

    Ok(HardwareProfileResponse {
        gpu_type: profile.gpu_type,
        vram_gb: profile.vram_gb,
        execution_provider: profile.execution_provider,
    })
}

#[tauri::command]
pub async fn start_transcription(state: State<'_, AppState>) -> Result<bool, ApiError> {
    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    let running = Arc::clone(&state.transcription_running);
    let _pool = state.pool.clone();

    tauri::async_runtime::spawn(async move {
        let config = AudioPipelineConfig::new(16_000, 1000);
        let engine = MockInferenceEngine::new(ExecutionProvider::Cpu, |_| Ok(vec![]));
        let mut pipeline = AudioPipeline::new(Box::new(engine), config);

        while running.load(Ordering::SeqCst) {
            let chunk_samples = (16_000 * 100 / 1000) as usize;
            let input = AudioInput {
                sample_rate: 16_000,
                channels: 1,
                samples: vec![0.0; chunk_samples],
            };
            let _ = pipeline.process_input(input);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });

    Ok(true)
}

#[tauri::command]
pub async fn stop_transcription(state: State<'_, AppState>) -> Result<bool, ApiError> {
    if state.try_stop_transcription() {
        Ok(false)
    } else {
        Err(ApiError::new(
            "transcription_not_running",
            "transcription not running",
        ))
    }
}

#[tauri::command]
pub async fn get_model_info(
    state: State<'_, AppState>,
    override_size: Option<String>,
) -> Result<ModelInfoResponse, ApiError> {
    let provider = SysinfoProvider::new();
    let profile = detect_profile(&provider);

    let override_size = match override_size {
        Some(value) => Some(parse_model_size(&value)?),
        None => {
            let guard = state.model_override.lock().await;
            *guard
        }
    };

    let selection = select_model(&profile, override_size);

    Ok(ModelInfoResponse {
        size: model_size_label(selection.size).to_string(),
        reason: selection.reason,
        execution_provider: profile.execution_provider,
    })
}

#[tauri::command]
pub async fn get_transcripts(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let repo = TranscriptRepository::new(state.pool.clone());
    let items = repo.list_recent(limit.unwrap_or(100)).await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

#[tauri::command]
pub async fn save_transcript(
    state: State<'_, AppState>,
    content: String,
) -> Result<TranscriptResponse, ApiError> {
    let repo = TranscriptRepository::new(state.pool.clone());
    let transcript = repo.create(CreateTranscript { content }).await?;
    Ok(transcript.into())
}

#[tauri::command]
pub async fn get_dictionary(
    state: State<'_, AppState>,
) -> Result<Vec<DictionaryResponse>, ApiError> {
    let repo = DictionaryRepository::new(state.pool.clone());
    let items = repo.list_all().await?;
    Ok(items.into_iter().map(DictionaryResponse::from).collect())
}

#[tauri::command]
pub async fn update_dictionary(
    state: State<'_, AppState>,
    term: String,
    replacement: String,
) -> Result<DictionaryResponse, ApiError> {
    let repo = DictionaryRepository::new(state.pool.clone());
    let entry = repo
        .upsert(CreateDictionaryEntry { term, replacement })
        .await?;
    Ok(entry.into())
}

fn parse_model_size(value: &str) -> Result<ModelSize, ApiError> {
    match value.to_lowercase().as_str() {
        "tiny" => Ok(ModelSize::Tiny),
        "small" => Ok(ModelSize::Small),
        "medium" => Ok(ModelSize::Medium),
        "large" => Ok(ModelSize::Large),
        _ => Err(ApiError::new("invalid_input", "invalid model size")),
    }
}

fn model_size_label(size: ModelSize) -> &'static str {
    match size {
        ModelSize::Tiny => "tiny",
        ModelSize::Small => "small",
        ModelSize::Medium => "medium",
        ModelSize::Large => "large",
    }
}

fn map_db_error(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(_) => "database error".to_string(),
        sqlx::Error::RowNotFound => "record not found".to_string(),
        sqlx::Error::PoolClosed => "database unavailable".to_string(),
        sqlx::Error::Io(_) => "database io error".to_string(),
        _ => "database error".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_size_parsing() {
        assert_eq!(parse_model_size("tiny").unwrap(), ModelSize::Tiny);
        assert_eq!(parse_model_size("SMALL").unwrap(), ModelSize::Small);
        assert!(parse_model_size("mega").is_err());
    }
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), ApiError> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .show()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
        window
            .set_focus()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_main_window(app: AppHandle) -> Result<(), ApiError> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .hide()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn type_text(text: String) -> Result<(), ApiError> {
    use enigo::{Enigo, Keyboard, Settings};

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let mut enigo = Enigo::new(&Settings::default()).unwrap();
        let _ = enigo.text(&text);
    });

    Ok(())
}

#[tauri::command]
pub async fn register_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<bool, ApiError> {
    use tauri::Emitter;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    // Unregister all existing shortcuts first
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| ApiError::new("hotkey_error", e.to_string()))?;

    // Register the new hotkey with handler (on_shortcut registers it implicitly)
    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey.as_str(), move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                let _ = app_clone.emit("hotkey-pressed", ());
            } else {
                let _ = app_clone.emit("hotkey-released", ());
            }
        })
        .map_err(|e| ApiError::new("hotkey_error", e.to_string()))?;

    // Store the hotkey in state
    let mut current = state.current_hotkey.lock().await;
    *current = Some(hotkey);

    Ok(true)
}

#[tauri::command]
pub async fn get_registered_hotkeys(state: State<'_, AppState>) -> Result<Vec<String>, ApiError> {
    let current = state.current_hotkey.lock().await;
    Ok(current
        .as_ref()
        .map(|h| vec![h.clone()])
        .unwrap_or_default())
}
