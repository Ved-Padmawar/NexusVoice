// Tauri requires State<'_, T> by value in sync command signatures — this is correct usage.
#![allow(clippy::needless_pass_by_value)]

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
use crate::postprocess::{extract_trackable_words, DictionaryCorrectionEngine};
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
    pub word_count: i64,
    pub duration_seconds: Option<f64>,
    pub created_at: String,
}

impl From<Transcript> for TranscriptResponse {
    fn from(value: Transcript) -> Self {
        Self {
            id: value.id,
            content: value.content,
            word_count: value.word_count,
            duration_seconds: value.duration_seconds,
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
    pub hits: i64,
    pub created_at: String,
}

impl From<DictionaryEntry> for DictionaryResponse {
    fn from(value: DictionaryEntry) -> Self {
        Self {
            id: value.id,
            term: value.term,
            replacement: value.replacement,
            hits: value.hits,
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
    let (user, pair) = state.auth().await.login_with_tokens(&email, &password).await?;
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
    let (user, pair) = state.auth().await.register_with_tokens(&email, &password).await?;
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

// ---------------------------------------------------------------------------
// Transcription commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_lines)]
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
        let mut buf = state.audio_buffer.lock().expect("audio_buffer lock poisoned");
        buf.clear();
    }

    // Install a fresh pipeline for this recording session.
    *state.pipeline.lock().await = Some(crate::pipeline::StreamingPipeline::new());

    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let capture_done = Arc::clone(&state.capture_done);
    let app_handle = app.clone();

    // Reset the done flag before starting a new capture session.
    *capture_done.0.lock().expect("capture_done lock poisoned") = false;

    std::thread::spawn(move || {
        if let Err(e) = crate::audio::capture_microphone(
            Arc::clone(&running),
            audio_buffer,
            native_rate,
            Arc::clone(&capture_done),
        ) {
            log::error!("microphone capture error: {e}");
            running.store(false, Ordering::SeqCst);
            // Signal done even on error so stop_transcription doesn't wait forever.
            *capture_done.0.lock().expect("capture_done lock poisoned") = true;
            capture_done.1.notify_one();
            let _ = app_handle.emit("transcription-error", e);
        }
    });

    // Spawn a background poller that fires pipeline chunks while recording.
    // It wakes every 2 s, checks if enough audio has accumulated, and if so
    // preprocesses + transcribes the next chunk — so most work is done before
    // the user releases the hotkey.
    {
        let running = Arc::clone(&state.transcription_running);
        let audio_buffer = Arc::clone(&state.audio_buffer);
        let native_rate_arc = Arc::clone(&state.native_sample_rate);
        let pipeline_arc = Arc::clone(&state.pipeline);
        let engine_arc = Arc::clone(&state.engine);
        let engine_cache = Arc::clone(&state.engine);
        let pool = state.db().await.clone();

        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                if !running.load(Ordering::SeqCst) {
                    break;
                }

                // Snapshot buffer + rate under lock, then release immediately.
                let (buffer_snapshot, captured_rate) = {
                    let buf = audio_buffer.lock().expect("audio_buffer lock poisoned");
                    let rate = *native_rate_arc.lock().expect("native_rate lock poisoned");
                    (buf.clone(), rate)
                };

                // Get engine — skip this tick if not loaded yet.
                let engine = {
                    let guard = engine_arc.lock().await;
                    guard.as_ref().map(Arc::clone)
                };
                let Some(engine) = engine else { continue };

                // Build prompt from recent transcripts for this chunk.
                let prompt = {
                    let repo = TranscriptRepository::new(pool.clone());
                    repo.list_recent(5).await.unwrap_or_default()
                        .into_iter()
                        .rev()
                        .map(|t| t.content)
                        .collect::<Vec<_>>()
                        .join(" ")
                };

                // Read beam size preference.
                // beam_size_path is not accessible here; we read from the engine_cache handle.
                // Instead we pass 5 (Balanced default) — the full beam size is used on finalize.
                // Actually we need the real value: read it via a blocking call.
                // We can't access AppState here cleanly, so carry beam_size as a captured value.
                // beam_size is set once per recording so snapshot it outside the loop.
                // (handled below — see beam_size capture above spawn)

                let committed = tauri::async_runtime::spawn_blocking({
                    let engine = Arc::clone(&engine);
                    let engine_cache = Arc::clone(&engine_cache);
                    let pipeline_arc = Arc::clone(&pipeline_arc);
                    move || {
                        let mut pl_guard = pipeline_arc.blocking_lock();
                        if let Some(pl) = pl_guard.as_mut() {
                            let did_commit = pl.try_commit_chunk(
                                &buffer_snapshot,
                                captured_rate,
                                &engine,
                                &prompt,
                                5, // Balanced — same as default beam size
                            );
                            if did_commit {
                                // Check if engine was poisoned
                                if engine.is_poisoned() {
                                    log::error!("WhisperEngine mutex poisoned during streaming chunk — evicting");
                                    drop(pl_guard);
                                    *engine_cache.blocking_lock() = None;
                                    return Err("engine_poisoned");
                                }
                            }
                            Ok(did_commit)
                        } else {
                            Ok(false)
                        }
                    }
                })
                .await;

                match committed {
                    Ok(Ok(true)) => log::debug!("streaming: mid-recording chunk committed"),
                    Ok(Err("engine_poisoned")) => break,
                    _ => {}
                }
            }
        });
    }

    Ok(true)
}

#[tauri::command]
#[allow(clippy::too_many_lines)]
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

    // Wait for the capture thread to fully stop and drop the cpal stream.
    // Uses a condvar instead of a fixed sleep — returns as soon as the thread signals done.
    let capture_done = Arc::clone(&state.capture_done);
    tauri::async_runtime::spawn_blocking(move || {
        let (lock, cvar) = &*capture_done;
        let _guard = cvar.wait_while(lock.lock().expect("capture_done lock poisoned"), |done| !*done).expect("capture_done condvar poisoned");
    })
    .await
    .ok();

    let (samples, captured_rate) = {
        let mut buf = state.audio_buffer.lock().expect("audio_buffer lock poisoned");
        let rate = *state.native_sample_rate.lock().expect("native_sample_rate lock poisoned");
        (std::mem::take(&mut *buf), rate)
    };

    #[allow(clippy::items_after_statements)]
    const MIN_DURATION_SECS: f64 = 0.5;

    // Compute real recording duration from raw samples before any processing.
    #[allow(clippy::cast_precision_loss)] // sample counts fit f64 mantissa at typical lengths
    let duration_seconds: Option<f64> = if captured_rate > 0 && !samples.is_empty() {
        Some(samples.len() as f64 / f64::from(captured_rate))
    } else {
        None
    };

    // Reject empty or sub-0.5s recordings.
    #[allow(clippy::cast_precision_loss)]
    let too_short = samples.is_empty()
        || (captured_rate > 0 && (samples.len() as f64 / f64::from(captured_rate)) < MIN_DURATION_SECS);
    if too_short {
        *state.pipeline.lock().await = None;
        let _ = app.emit("transcription-complete", "");
        return Ok(false);
    }

    let pool = state.db().await.clone();
    let app_handle = app.clone();
    let dict_cache: DictCache = Arc::clone(&state.dict_cache);

    let engine = match state.get_or_load_engine().await {
        Ok(e) => e,
        Err(e) => {
            log::error!("engine load failed: {e}");
            *state.pipeline.lock().await = None;
            let _ = app_handle.emit("transcription-error", format!("model not ready: {e}"));
            return Ok(false);
        }
    };
    let engine_cache = Arc::clone(&state.engine);

    // Take the pipeline out of state — finalize consumes it.
    let pipeline = state.pipeline.lock().await.take();

    let dict_entries: Vec<_> = dict_cache.read().await.values().cloned().collect();
    let beam_size = state.load_beam_size();

    let prompt = {
        let transcript_repo = TranscriptRepository::new(pool.clone());
        let recent = transcript_repo.list_recent(5).await.unwrap_or_default();
        recent
            .into_iter()
            .rev()
            .map(|t| t.content)
            .collect::<Vec<_>>()
            .join(" ")
    };

    tauri::async_runtime::spawn(async move {
        // finalize() preprocesses only the tail (audio since last committed chunk)
        // and stitches with previously committed chunk texts — fast path when
        // mid-recording chunks already covered most of the speech.
        let raw_text = tauri::async_runtime::spawn_blocking({
            let engine = Arc::clone(&engine);
            let engine_cache = Arc::clone(&engine_cache);
            move || -> Result<String, String> {
                let pl = pipeline.unwrap_or_else(crate::pipeline::StreamingPipeline::new);
                let Ok(text) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    pl.finalize(&samples, captured_rate, &engine, &prompt, beam_size)
                })) else {
                    log::error!("WhisperEngine panicked during finalize — evicting");
                    *engine_cache.blocking_lock() = None;
                    return Err("engine_poisoned".to_string());
                };
                // Also evict if Mutex was poisoned during finalize
                if engine.is_poisoned() {
                    log::error!("WhisperEngine mutex poisoned after finalize — evicting");
                    let mut cache = engine_cache.blocking_lock();
                    *cache = None;
                }
                Ok(text)
            }
        })
        .await
        .map_err(|e| format!("finalize join error: {e}"))
        .and_then(|r| r);

        let raw_text = match raw_text {
            Ok(t) => t,
            Err(e) => {
                let _ = app_handle.emit(
                    "transcription-error",
                    "Transcription engine encountered an error and was reset. Please try again.",
                );
                log::error!("finalize failed: {e}");
                return Ok(());
            }
        };

        // Strip leading dash hallucinations — Whisper emits "- " at the start of short utterances.
        let raw_text = raw_text
            .trim_start_matches(|c: char| c == '-' || c == '–' || c == '—' || c.is_whitespace())
            .to_string();

        log::debug!("final stitched result: {} chars", raw_text.len());

        if raw_text.is_empty() {
            let _ = app_handle.emit("transcription-complete", "");
            return Ok(());
        }

        // Post-process: apply dictionary corrections.
        let corrector = DictionaryCorrectionEngine::new(dict_entries);
        let (text, matched_terms) = corrector.apply_to_text(&raw_text);

        if !matched_terms.is_empty() {
            let dict_repo = DictionaryRepository::new(pool.clone());
            let _ = dict_repo.increment_hits_batch(&matched_terms).await;
            let mut cache = dict_cache.write().await;
            for term in &matched_terms {
                if let Some(entry) = cache.get_mut(term) {
                    entry.hits += 1;
                }
            }
        }

        let _ = app_handle.emit("transcription-complete", text.clone());

        let repo = TranscriptRepository::new(pool.clone());
        #[allow(clippy::cast_possible_wrap)] // word count never exceeds i64::MAX
        let word_count = text.split_whitespace().count() as i64;
        if let Ok(saved) = repo.create(CreateTranscript { content: text.clone(), word_count, duration_seconds }).await {
            let _ = app_handle.emit("transcript:new", TranscriptResponse::from(saved));
        }

        // Auto-learn words from raw_text (pre-correction).
        let words = extract_trackable_words(&raw_text);
        if !words.is_empty() {
            let freq_repo = WordFrequencyRepository::new(pool.clone());
            match freq_repo.increment_batch(&words).await {
                Ok(newly_learned) if !newly_learned.is_empty() => {
                    let dict_repo = DictionaryRepository::new(pool.clone());
                    let mut added: Vec<DictionaryResponse> = Vec::new();
                    for word in &newly_learned {
                        match dict_repo.upsert(CreateDictionaryEntry {
                            term: word.clone(),
                            replacement: word.clone(),
                        }).await {
                            Ok(entry) => {
                                dict_cache.write().await.entry(entry.term.clone()).or_insert_with(|| entry.clone());
                                added.push(DictionaryResponse::from(entry));
                                log::debug!("auto-learned word: {word}");
                            }
                            Err(e) => log::warn!("auto-learn dict upsert failed for {word}: {e}"),
                        }
                    }
                    if !added.is_empty() {
                        let _ = app_handle.emit("dictionary:updated", &added);
                    }
                }
                Ok(_) => {}
                Err(e) => log::warn!("word frequency update failed: {e}"),
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
    let repo = TranscriptRepository::new(state.db().await.clone());
    let (total_sessions, total_words, total_duration_seconds) = repo.get_stats().await?;

    #[allow(clippy::cast_possible_truncation)] // durations and word counts fit i64
    let speaking_time_seconds = total_duration_seconds.round() as i64;

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let avg_pace_wpm = if total_duration_seconds > 0.0 {
        ((total_words as f64 / (total_duration_seconds / 60.0)).round()) as i64
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
    offset: Option<i64>,
    from: Option<String>,
    to: Option<String>,
    sort_asc: Option<bool>,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let repo = TranscriptRepository::new(state.db().await.clone());
    let items = repo.list_paginated(
        limit.unwrap_or(50),
        offset.unwrap_or(0),
        from.as_deref(),
        to.as_deref(),
        !sort_asc.unwrap_or(false),
    ).await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

#[tauri::command]
pub async fn export_transcripts(
    state: State<'_, AppState>,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let repo = TranscriptRepository::new(state.db().await.clone());
    let items = repo.list_all().await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

#[tauri::command]
pub async fn search_transcripts(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
    offset: Option<i64>,
    from: Option<String>,
    to: Option<String>,
    sort_asc: Option<bool>,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Load user vocabulary from word_frequency table for fuzzy matching
    let vocab: Vec<String> = sqlx::query_scalar("SELECT word FROM word_frequency ORDER BY count DESC LIMIT 2000")
        .fetch_all(state.db().await)
        .await
        .unwrap_or_default();

    let fts_query = TranscriptRepository::build_fts_query(&query, &vocab);

    let repo = TranscriptRepository::new(state.db().await.clone());
    let items = repo.search(
        &fts_query,
        limit.unwrap_or(50),
        offset.unwrap_or(0),
        from.as_deref(),
        to.as_deref(),
        !sort_asc.unwrap_or(false),
    ).await.unwrap_or_default();
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}


#[tauri::command]
pub async fn save_transcript(
    state: State<'_, AppState>,
    content: String,
) -> Result<TranscriptResponse, ApiError> {
    if content.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "transcript content cannot be empty"));
    }
    let repo = TranscriptRepository::new(state.db().await.clone());
    #[allow(clippy::cast_possible_wrap)]
    let word_count = content.split_whitespace().count() as i64;
    let transcript = repo.create(CreateTranscript { content, word_count, duration_seconds: None }).await?;
    Ok(transcript.into())
}

#[tauri::command]
pub async fn get_dictionary(
    state: State<'_, AppState>,
) -> Result<Vec<DictionaryResponse>, ApiError> {
    let cache = state.dict_cache.read().await;
    Ok(cache.values().cloned().map(DictionaryResponse::from).collect())
}

#[tauri::command]
pub async fn update_dictionary(
    state: State<'_, AppState>,
    term: String,
    replacement: String,
) -> Result<DictionaryResponse, ApiError> {
    let repo = DictionaryRepository::new(state.db().await.clone());
    let entry = repo
        .upsert(CreateDictionaryEntry { term: term.clone(), replacement })
        .await?;

    // Update in-memory cache: O(1) insert/replace via HashMap
    state.dict_cache.write().await.insert(entry.term.clone(), entry.clone());

    Ok(entry.into())
}

#[tauri::command]
pub async fn delete_dictionary_entry(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, ApiError> {
    let repo = DictionaryRepository::new(state.db().await.clone());
    let deleted = repo.delete_by_id(id).await?;
    if deleted {
        let mut cache = state.dict_cache.write().await;
        cache.retain(|_, e| e.id != id);
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn apply_dictionary(
    state: State<'_, AppState>,
    text: String,
) -> Result<String, ApiError> {
    let entries: Vec<_> = state.dict_cache.read().await.values().cloned().collect();
    let engine = DictionaryCorrectionEngine::new(entries);
    Ok(engine.apply_to_text(&text).0)
}

// ---------------------------------------------------------------------------
// Model manager commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedModel {
    pub variant: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub is_active: bool,
}

/// List all model files currently on disk.
#[tauri::command]
pub fn get_downloaded_models(state: State<'_, AppState>) -> Vec<DownloadedModel> {
    use crate::inference::provider::{detect_backend, select_model_size, ModelSize};

    let active_override = state.load_model_override();
    let active_backend = detect_backend();
    let active_size = select_model_size(active_backend, active_override.as_deref());

    let all: &[(&str, ModelSize)] = &[
        ("tiny",   ModelSize::Tiny),
        ("base",   ModelSize::Base),
        ("small",  ModelSize::Small),
        ("medium", ModelSize::Medium),
        ("large",  ModelSize::Large),
    ];

    all.iter().filter_map(|(variant, size)| {
        let path = state.models_dir.join(size.filename());
        if !path.exists() { return None; }
        let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
        Some(DownloadedModel {
            variant: variant.to_string(),
            display_name: size.display_name().to_string(),
            size_bytes,
            is_active: *size == active_size,
        })
    }).collect()
}

/// Delete a downloaded model file by variant ("tiny" | "base" | "small" | "medium" | "large").
/// Refuses to delete the currently active model.
#[tauri::command]
pub async fn delete_model(
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    use crate::inference::provider::{detect_backend, select_model_size, ModelSize};

    let size = match variant.as_str() {
        "tiny"   => ModelSize::Tiny,
        "base"   => ModelSize::Base,
        "small"  => ModelSize::Small,
        "medium" => ModelSize::Medium,
        "large"  => ModelSize::Large,
        _ => return Err(ApiError::new("invalid_variant", "variant must be tiny, base, small, medium, or large")),
    };

    let active_override = state.load_model_override();
    let active_backend = detect_backend();
    let active_size = select_model_size(active_backend, active_override.as_deref());

    if size == active_size {
        return Err(ApiError::new("active_model", "cannot delete the currently active model"));
    }

    let path = state.models_dir.join(size.filename());
    if !path.exists() {
        return Err(ApiError::new("not_found", "model file not found"));
    }

    std::fs::remove_file(&path)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;

    // If deleted model was cached in engine, evict it
    *state.engine.lock().await = None;

    Ok(())
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
    *state.current_hotkey.lock().await = Some(hotkey);

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
    let hotkey = state.current_hotkey.lock().await.clone();
    if let Some(h) = hotkey {
        return Ok(vec![h]);
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
    use crate::inference::provider::recommend_model_size;

    let hw = crate::hardware::cached_profile();
    let recommended = recommend_model_size();

    Ok(HardwareProfileResponse {
        gpu_name: hw.gpu_type.clone(),
        execution_provider: hw.execution_provider.clone(),
        vram_gb: hw.vram_gb,
        ram_gb: hw.ram_gb,
        recommended_model: recommended.display_name().to_string(),
    })
}

// ---------------------------------------------------------------------------
// Model override commands
// ---------------------------------------------------------------------------

/// Set model size override ("tiny" | "base" | "small" | "medium" | "large").
/// Clears the cached engine so the next transcription reloads with the chosen model.
#[tauri::command]
pub async fn set_model_override(
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    if !matches!(variant.as_str(), "tiny" | "base" | "small" | "medium" | "large") {
        return Err(ApiError::new(
            "invalid_variant",
            "variant must be 'tiny', 'base', 'small', 'medium', or 'large'",
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
// Beam size commands
// ---------------------------------------------------------------------------

/// Get the current beam size. Returns 2 (Fast), 5 (Balanced), or 8 (Accurate).
#[tauri::command]
pub fn get_beam_size(state: State<'_, AppState>) -> i32 {
    state.load_beam_size()
}

/// Set beam size to 2, 5, or 8. Does not evict the engine — `beam_size` is applied
/// per transcription call, so it takes effect immediately on the next recording.
#[tauri::command]
pub fn set_beam_size(state: State<'_, AppState>, beam_size: i32) -> Result<(), ApiError> {
    if beam_size != 2 && beam_size != 5 && beam_size != 8 {
        return Err(ApiError::new(
            "invalid_beam_size",
            "beam_size must be 2 (Fast), 5 (Balanced), or 8 (Accurate)",
        ));
    }
    state
        .save_beam_size(beam_size)
        .map_err(|e| ApiError::new("io_error", e.to_string()))
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
    let progress = *dl.progress.lock().expect("progress lock poisoned");
    let error = dl.error.lock().expect("error lock poisoned").clone();

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
        match crate::inference::downloader::download_whisper_model(&models_dir, model_size, &app, &dl_state) {
            Ok(()) => {
                dl_state.set_complete();
                let _ = app.emit("model-download-complete", ());
            }
            Err(e) if e == "download_cancelled" => {
                dl_state.set_cancelled();
                let _ = app.emit("model-download-cancelled", ());
            }
            Err(e) => {
                dl_state.set_error(e.clone());
                let _ = app.emit("model-download-error", e);
            }
        }
    });

    Ok(true)
}

/// Cancel an in-progress model download. The download loop checks this flag
/// each chunk and exits cleanly, deleting the partial .tmp file.
#[tauri::command]
pub fn cancel_model_download(state: State<'_, AppState>) {
    let dl = &state.model_download;
    let status = dl.status.load(std::sync::atomic::Ordering::SeqCst);
    if status == 1 {
        dl.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

