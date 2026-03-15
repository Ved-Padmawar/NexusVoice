use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::auth::{AuthError, TokenPair};
use crate::database::dto::{dictionary::CreateDictionaryEntry, transcript::CreateTranscript};
use crate::database::models::{dictionary::DictionaryEntry, transcript::Transcript, user::User};
use crate::database::repositories::{
    dictionary::DictionaryRepository, transcript::TranscriptRepository,
};
use crate::database::repositories::word_frequency::WordFrequencyRepository;
use crate::postprocess::{extract_trackable_words, DictionaryCorrectionConfig, DictionaryCorrectionEngine};
use crate::state::{AppState, DictCache};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

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

fn map_db_error(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(_) => "database error".to_string(),
        sqlx::Error::RowNotFound => "record not found".to_string(),
        sqlx::Error::PoolClosed => "database unavailable".to_string(),
        sqlx::Error::Io(_) => "database io error".to_string(),
        _ => "database error".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Serialize)]
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
pub struct AuthStateResponse {
    pub authenticated: bool,
    pub user_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// Auth commands
// ---------------------------------------------------------------------------

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
    let repo = crate::database::repositories::user::UserRepository::new(state.pool.clone());
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
        let _ = state.auth.revoke_token(&token).await;
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

// ---------------------------------------------------------------------------
// Transcription commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    if state.current_user_id().await.is_none() {
        return Err(ApiError::new("unauthenticated", "must be logged in to transcribe"));
    }

    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    {
        let mut buf = state.audio_buffer.lock().unwrap();
        buf.clear();
    }

    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let app_handle = app.clone();

    std::thread::spawn(move || {
        if let Err(e) =
            crate::audio::capture_microphone(Arc::clone(&running), audio_buffer, native_rate)
        {
            log::error!("microphone capture error: {e}");
            running.store(false, Ordering::SeqCst);
            let _ = app_handle.emit("transcription-error", e);
        }
    });

    Ok(true)
}

#[tauri::command]
pub async fn stop_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    if !state.try_stop_transcription() {
        return Err(ApiError::new(
            "transcription_not_running",
            "transcription not running",
        ));
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    let (samples, captured_rate) = {
        let buf = state.audio_buffer.lock().unwrap();
        let rate = *state.native_sample_rate.lock().unwrap();
        (buf.clone(), rate)
    };

    if samples.is_empty() {
        let _ = app.emit("transcription-complete", "");
        return Ok(false);
    }

    let pool = state.pool.clone();
    let app_handle = app.clone();
    let dict_cache: DictCache = Arc::clone(&state.dict_cache);

    let engine = match state.get_or_load_engine().await {
        Ok(e) => e,
        Err(e) => {
            log::error!("engine load failed: {e}");
            let _ = app_handle.emit("transcription-error", format!("model not ready: {e}"));
            return Ok(false);
        }
    };

    // Snapshot dictionary cache once — used for both prompt and post-processing
    let dict_entries = dict_cache.read().await.clone();

    // Build Whisper initial prompt from:
    //   1. Tail of recent transcripts (last 5, joined) — gives personal vocab context
    //   2. Dictionary replacement terms appended at the end — biases toward known spellings
    // Combined and capped; the decoder will take the last 223 tokens if over the limit.
    let prompt = {
        let transcript_repo = TranscriptRepository::new(pool.clone());
        let recent = transcript_repo.list_recent(5).await.unwrap_or_default();

        let mut parts: Vec<String> = recent
            .into_iter()
            .rev() // oldest first so context reads naturally
            .map(|t| t.content)
            .collect();

        // Append dictionary replacements as a comma-separated hint at the end
        if !dict_entries.is_empty() {
            let terms: Vec<String> = dict_entries.iter().map(|e| e.replacement.clone()).collect();
            parts.push(terms.join(", "));
        }

        parts.join(" ")
    };

    tauri::async_runtime::spawn(async move {
        log::debug!("preprocessing {} samples at {}Hz", samples.len(), captured_rate);
        let resampled = tauri::async_runtime::spawn_blocking(move || {
            crate::preprocess::preprocess(&samples, captured_rate)
        })
        .await
        .map_err(|e| format!("preprocess join error: {e}"))?;

        log::debug!("resampled to {} samples, chunking for inference", resampled.len());

        // Split into VAD-aware chunks (no-op for recordings ≤ 30 s)
        let chunks = crate::preprocess::chunker::chunk_audio(&resampled);
        log::debug!("audio split into {} chunk(s)", chunks.len());

        // Transcribe each chunk sequentially (engine holds the model in a Mutex)
        let mut chunk_texts: Vec<String> = Vec::with_capacity(chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_samples = chunk.samples.clone();
            let chunk_prompt = prompt.clone();
            let eng = engine.clone();
            let chunk_result = tauri::async_runtime::spawn_blocking(move || {
                let mut eng = eng.lock().unwrap_or_else(|e| e.into_inner());
                eng.transcribe(&chunk_samples, &chunk_prompt)
            })
            .await
            .map_err(|e| format!("inference join error (chunk {i}): {e}"))
            .and_then(|r| r);

            match chunk_result {
                Ok(text) => {
                    log::debug!("chunk {i} → {} chars", text.len());
                    chunk_texts.push(text);
                }
                Err(e) => {
                    log::warn!("chunk {i} inference failed: {e}");
                }
            }
        }

        // Stitch all chunk transcripts into the final text
        let raw_text = crate::preprocess::chunker::stitch_transcripts(&chunk_texts);
        log::debug!("stitched result: {} chars", raw_text.len());

        let result: Result<String, String> = if raw_text.is_empty() {
            Ok(String::new())
        } else {
            Ok(raw_text)
        };

        match result {
            Ok(raw_text) if !raw_text.is_empty() => {
                // Post-process: apply dictionary corrections to whisper output
                let corrector = DictionaryCorrectionEngine::new(
                    dict_entries,
                    DictionaryCorrectionConfig::default(),
                );
                let text = corrector.apply_to_text(&raw_text);

                let _ = app_handle.emit("transcription-complete", text.clone());
                let repo = TranscriptRepository::new(pool.clone());
                if let Ok(saved) = repo.create(CreateTranscript { content: text.clone() }).await {
                    let _ = app_handle.emit("transcript:new", TranscriptResponse::from(saved));
                }

                // Auto-learn words: track frequency and add to dictionary once threshold is hit.
                // Use raw_text (pre-correction) so dictionary replacements don't pollute counts.
                let words = extract_trackable_words(&raw_text);
                if !words.is_empty() {
                    let freq_repo = WordFrequencyRepository::new(pool.clone());
                    match freq_repo.increment_batch(&words).await {
                        Ok(newly_learned) if !newly_learned.is_empty() => {
                            let dict_repo = DictionaryRepository::new(pool.clone());
                            for word in &newly_learned {
                                match dict_repo.upsert(CreateDictionaryEntry {
                                    term: word.clone(),
                                    replacement: word.clone(),
                                }).await {
                                    Ok(entry) => {
                                        let mut cache = dict_cache.write().await;
                                        if !cache.iter().any(|e| e.term == entry.term) {
                                            cache.push(entry);
                                        }
                                        log::debug!("auto-learned word: {word}");
                                    }
                                    Err(e) => log::warn!("auto-learn dict upsert failed for {word}: {e}"),
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(e) => log::warn!("word frequency update failed: {e}"),
                    }
                }
            }
            Ok(_) => {
                let _ = app_handle.emit("transcription-complete", "");
            }
            Err(e) => {
                let _ = app_handle.emit("transcription-error", e);
            }
        }

        Ok::<(), String>(())
    });

    Ok(true)
}

// ---------------------------------------------------------------------------
// Text injection
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn type_text(app: AppHandle, text: String) -> Result<(), ApiError> {
    use enigo::{Enigo, Key, Keyboard, Settings};
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .write_text(text)
        .map_err(|e| ApiError::new("clipboard_error", e.to_string()))?;

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            let _ = enigo.key(Key::Control, enigo::Direction::Press);
            let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
            let _ = enigo.key(Key::Control, enigo::Direction::Release);
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Data commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatsResponse {
    pub total_words: i64,
    pub speaking_time_seconds: i64,
    pub total_sessions: i64,
    pub avg_pace_wpm: i64,
}

#[tauri::command]
pub async fn get_usage_stats(state: State<'_, AppState>) -> Result<UsageStatsResponse, ApiError> {
    let repo = TranscriptRepository::new(state.pool.clone());
    let transcripts = repo.list_recent(10_000).await?;

    let total_sessions = transcripts.len() as i64;
    let total_words: i64 = transcripts
        .iter()
        .map(|t| t.content.split_whitespace().count() as i64)
        .sum();
    let speaking_time_seconds = (total_words as f64 * 60.0 / 130.0).round() as i64;
    let avg_pace_wpm = if total_sessions > 0 {
        total_words / total_sessions.max(1)
    } else {
        0
    };

    Ok(UsageStatsResponse {
        total_words,
        speaking_time_seconds,
        total_sessions,
        avg_pace_wpm,
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
    let cache = state.dict_cache.read().await;
    Ok(cache.iter().cloned().map(DictionaryResponse::from).collect())
}

#[tauri::command]
pub async fn update_dictionary(
    state: State<'_, AppState>,
    term: String,
    replacement: String,
) -> Result<DictionaryResponse, ApiError> {
    let repo = DictionaryRepository::new(state.pool.clone());
    let entry = repo
        .upsert(CreateDictionaryEntry { term: term.clone(), replacement })
        .await?;

    // Update in-memory cache: replace existing entry or append
    let mut cache = state.dict_cache.write().await;
    if let Some(existing) = cache.iter_mut().find(|e| e.term == term) {
        *existing = entry.clone();
    } else {
        cache.push(entry.clone());
    }

    Ok(entry.into())
}

#[tauri::command]
pub async fn delete_dictionary_entry(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, ApiError> {
    let repo = DictionaryRepository::new(state.pool.clone());
    let deleted = repo.delete_by_id(id).await?;
    if deleted {
        let mut cache = state.dict_cache.write().await;
        cache.retain(|e| e.id != id);
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn apply_dictionary(
    state: State<'_, AppState>,
    text: String,
) -> Result<String, ApiError> {
    let entries = state.dict_cache.read().await.clone();
    let engine = DictionaryCorrectionEngine::new(entries, DictionaryCorrectionConfig::default());
    Ok(engine.apply_to_text(&text))
}

// ---------------------------------------------------------------------------
// Logs command
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn open_logs_folder(app: AppHandle) -> Result<(), ApiError> {
    let logs_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| ApiError::new("path_error", e.to_string()))?;
    std::fs::create_dir_all(&logs_dir)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
    opener::open(&logs_dir)
        .map_err(|e| ApiError::new("open_error", e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Window commands
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Hotkey commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn register_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<bool, ApiError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err(ApiError::new("hotkey_invalid", "hotkey cannot be empty"));
    }
    let parts: Vec<&str> = hotkey.split('+').collect();
    let modifiers = ["Ctrl", "Alt", "Shift", "Super", "Win"];
    let has_modifier = parts.iter().any(|p| modifiers.contains(p));
    let has_key = parts.iter().any(|p| !modifiers.contains(p));
    if !has_modifier || !has_key {
        return Err(ApiError::new(
            "hotkey_invalid",
            "hotkey must include at least one modifier (Ctrl/Alt/Shift) and one key",
        ));
    }

    app.global_shortcut()
        .unregister_all()
        .map_err(|e| ApiError::new("hotkey_unregister_failed", e.to_string()))?;

    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey.as_str(), move |_app, _shortcut, event| {
            use tauri_plugin_global_shortcut::ShortcutState;
            if event.state == ShortcutState::Pressed {
                let _ = app_clone.emit("hotkey-pressed", ());
            } else {
                let _ = app_clone.emit("hotkey-released", ());
            }
        })
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("already registered") || msg.contains("already in use") {
                ApiError::new(
                    "hotkey_already_in_use",
                    "this hotkey is already in use by another application",
                )
            } else if msg.contains("permission") || msg.contains("access") {
                ApiError::new(
                    "hotkey_permission_denied",
                    "OS denied hotkey registration — try a different combination",
                )
            } else {
                ApiError::new("hotkey_register_failed", msg)
            }
        })?;

    let _ = state.save_hotkey(&hotkey);
    let mut current = state.current_hotkey.lock().await;
    *current = Some(hotkey);

    Ok(true)
}

#[tauri::command]
pub async fn unregister_hotkey(app: AppHandle, state: State<'_, AppState>) -> Result<(), ApiError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    app.global_shortcut()
        .unregister_all()
        .map_err(|e| ApiError::new("hotkey_unregister_failed", e.to_string()))?;

    state.delete_hotkey();
    *state.current_hotkey.lock().await = None;

    Ok(())
}

#[tauri::command]
pub async fn get_registered_hotkeys(state: State<'_, AppState>) -> Result<Vec<String>, ApiError> {
    let current = state.current_hotkey.lock().await;
    if let Some(h) = current.as_ref() {
        return Ok(vec![h.clone()]);
    }
    Ok(state.load_hotkey().map(|h| vec![h]).unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Hardware commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileResponse {
    pub gpu_name: String,
    pub execution_provider: String,
    pub vram_gb: f32,
    pub ram_gb: f32,
    pub recommended_model: String,
}

#[tauri::command]
pub async fn get_hardware_profile() -> Result<HardwareProfileResponse, ApiError> {
    use crate::hardware::detector::detect_profile;
    use crate::hardware::sysinfo_provider::SysinfoProvider;
    use crate::inference::provider::recommend_model_size;

    let hw = detect_profile(&SysinfoProvider);
    let recommended = recommend_model_size();

    Ok(HardwareProfileResponse {
        gpu_name: hw.gpu_type,
        execution_provider: hw.execution_provider,
        vram_gb: hw.vram_gb,
        ram_gb: hw.ram_gb,
        recommended_model: recommended.display_name().to_string(),
    })
}

// ---------------------------------------------------------------------------
// Model override commands
// ---------------------------------------------------------------------------

/// Set model size override ("large" | "medium"). Clears the cached engine
/// so the next transcription reloads with the chosen model.
#[tauri::command]
pub async fn set_model_override(
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    if variant != "large" && variant != "medium" && variant != "small" {
        return Err(ApiError::new(
            "invalid_variant",
            "variant must be 'large', 'medium', or 'small'",
        ));
    }
    state
        .save_model_override(&variant)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
    *state.engine.lock().await = None;
    Ok(())
}

/// Clear the model size override, reverting to auto-selection based on hardware.
#[tauri::command]
pub async fn clear_model_override(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.delete_model_override();
    *state.engine.lock().await = None;
    Ok(())
}

// ---------------------------------------------------------------------------
// Model commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoResponse {
    pub downloaded: bool,
    pub downloading: bool,
    pub download_progress: u8,
    pub download_error: Option<String>,
    pub model_name: String,
}

#[tauri::command]
pub async fn get_model_info(state: State<'_, AppState>) -> Result<ModelInfoResponse, ApiError> {
    use crate::inference::provider::{detect_backend, select_model_size};

    let override_size = state.load_model_override();
    let backend = detect_backend();
    let model_size = select_model_size(backend, override_size.as_deref());
    let model_path = state.models_dir.join(model_size.filename());

    let dl = &state.model_download;
    let status = dl.status.load(std::sync::atomic::Ordering::SeqCst);
    let progress = *dl.progress.lock().unwrap();
    let error = dl.error.lock().unwrap().clone();

    Ok(ModelInfoResponse {
        downloaded: model_path.exists() || status == 2,
        downloading: status == 1,
        download_progress: progress,
        download_error: error,
        model_name: model_size.display_name().to_string(),
    })
}

#[tauri::command]
pub async fn retry_model_download(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    use crate::inference::provider::{detect_backend, select_model_size};

    let dl = &state.model_download;
    let status = dl.status.load(std::sync::atomic::Ordering::SeqCst);

    if status == 1 {
        return Err(ApiError::new(
            "already_downloading",
            "model download already in progress",
        ));
    }

    let override_size = state.load_model_override();
    let backend = detect_backend();
    let model_size = select_model_size(backend, override_size.as_deref());
    let model_path = state.models_dir.join(model_size.filename());

    if model_path.exists() {
        dl.set_complete();
        return Ok(true);
    }

    let dl_state = Arc::clone(&state.model_download);
    let models_dir = state.models_dir.clone();
    dl_state.set_downloading();
    let _ = app.emit("model-download-start", ());

    tauri::async_runtime::spawn_blocking(move || {
        match download_whisper_model(&models_dir, model_size, &app, &dl_state) {
            Ok(_) => {
                dl_state.set_complete();
                let _ = app.emit("model-download-complete", ());
            }
            Err(e) => {
                dl_state.set_error(e.clone());
                let _ = app.emit("model-download-error", e);
            }
        }
    });

    Ok(true)
}

/// Download the selected ggml model file. Reports progress via events.
pub fn download_whisper_model(
    models_dir: &std::path::Path,
    model_size: crate::inference::provider::ModelSize,
    app: &AppHandle,
    dl_state: &crate::state::ModelDownloadState,
) -> Result<(), String> {
    let file_urls: &[(&str, &str)] = &[(model_size.filename(), model_size.url())];

    // HEAD each URL to get Content-Length; use on-disk size for already-downloaded files
    let client = reqwest::blocking::Client::new();
    let mut file_sizes: Vec<u64> = Vec::new();
    for (filename, url) in file_urls {
        let dest = models_dir.join(filename);
        if dest.exists() {
            file_sizes.push(dest.metadata().map(|m| m.len()).unwrap_or(0));
        } else {
            let size = client
                .head(*url)
                .send()
                .ok()
                .and_then(|r| r.headers().get("content-length")?.to_str().ok()?.parse().ok())
                .unwrap_or(0);
            file_sizes.push(size);
        }
    }

    let total_bytes: u64 = file_sizes.iter().sum();
    let mut downloaded_total: u64 = 0;

    for ((filename, url), &size) in file_urls.iter().zip(file_sizes.iter()) {
        let dest = models_dir.join(filename);
        if dest.exists() {
            downloaded_total += size;
            continue;
        }
        download_file(url, &dest, app, dl_state, &mut downloaded_total, total_bytes)?;
    }

    Ok(())
}

fn download_file(
    url: &str,
    dest: &std::path::Path,
    app: &AppHandle,
    dl_state: &crate::state::ModelDownloadState,
    downloaded_total: &mut u64,
    total_bytes: u64,
) -> Result<(), String> {
    use std::io::{Read, Write};

    let mut response =
        reqwest::blocking::get(url).map_err(|e| format!("download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("download HTTP {} for {}", response.status(), url));
    }

    let tmp = dest.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("create file failed: {e}"))?;

    let mut last_pct: u8 = 0;
    let mut buf = vec![0u8; 256 * 1024]; // 256 KB chunks
    loop {
        let n = response
            .read(&mut buf)
            .map_err(|e| format!("read body failed: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("write file failed: {e}"))?;
        *downloaded_total += n as u64;
        if total_bytes > 0 {
            let pct = ((*downloaded_total * 100) / total_bytes).min(100) as u8;
            if pct != last_pct {
                last_pct = pct;
                dl_state.set_progress(pct);
                let _ = app.emit("model-download-progress", pct);
            }
        }
    }

    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| format!("rename file failed: {e}"))?;

    Ok(())
}
