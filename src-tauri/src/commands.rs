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
use crate::hardware::{detect_profile, SysinfoProvider};
use crate::inference::ExecutionProvider;
use crate::models::{select_model, ModelSize};
use crate::postprocess::module::dictionary_engine::{
    DictionaryCorrectionConfig, DictionaryCorrectionEngine,
};
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

/// Guard: returns the current user_id if authenticated, or an ApiError if not.
#[allow(dead_code)]
pub async fn require_auth(state: &AppState) -> Result<i64, ApiError> {
    state
        .current_user_id()
        .await
        .ok_or_else(|| ApiError::new("unauthenticated", "authentication required"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStateResponse {
    pub authenticated: bool,
    pub user_id: Option<i64>,
}

/// Returns the current auth state without requiring authentication.
/// Used by the frontend to check session on startup.
#[tauri::command]
pub async fn get_auth_state(state: State<'_, AppState>) -> Result<AuthStateResponse, ApiError> {
    let user_id = state.current_user_id().await;
    Ok(AuthStateResponse {
        authenticated: user_id.is_some(),
        user_id,
    })
}

/// Called by frontend after a successful login to persist the refresh token for next startup.
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

/// Called by frontend on logout to clear the persisted token and session.
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareTierResponse {
    pub tier: String,
    pub execution_provider: String,
    pub vram_gb: f32,
}

/// Returns the hardware tier (low/mid/high) derived from the detected profile.
#[tauri::command]
pub async fn get_hardware_tier() -> Result<HardwareTierResponse, ApiError> {
    let provider = SysinfoProvider::new();
    let profile = detect_profile(&provider);
    let tier = match profile.execution_provider.as_str() {
        "cuda" => "high",
        "directml" | "metal" => "mid",
        _ => "low",
    }
    .to_string();
    Ok(HardwareTierResponse {
        tier,
        execution_provider: profile.execution_provider,
        vram_gb: profile.vram_gb,
    })
}

/// Persist the model size override, update state, and pre-download the model if not cached.
#[tauri::command]
pub async fn set_model_override(
    app: AppHandle,
    state: State<'_, AppState>,
    size: String,
) -> Result<(), ApiError> {
    let model_size = parse_model_size(&size)?;
    let _ = state.save_model_override(&size);
    {
        let mut guard = state.model_override.lock().await;
        *guard = Some(model_size);
    }

    // Pre-download the model immediately so it's ready when the user records.
    let model_filename = model_size_to_filename(model_size);
    let model_path = state.models_dir.join(model_filename);

    if !model_path.exists() {
        let app_handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = app_handle.emit("model-download-start", ());
            match download_whisper_model(&model_path, &app_handle) {
                Ok(_) => { let _ = app_handle.emit("model-download-complete", ()); }
                Err(e) => { let _ = app_handle.emit("model-download-error", e); }
            }
        });
    }

    Ok(())
}

/// Clear the model override — revert to auto-selection.
#[tauri::command]
pub async fn clear_model_override(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.delete_model_override();
    let mut guard = state.model_override.lock().await;
    *guard = None;
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
pub async fn start_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    // Clear old audio data
    {
        let mut buf = state.audio_buffer.lock().unwrap();
        buf.clear();
    }

    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let app_handle = app.clone();

    std::thread::spawn(move || {
        if let Err(e) = capture_microphone(Arc::clone(&running), audio_buffer, native_rate) {
            eprintln!("microphone capture error: {e}");
            // Reset flag so the next start_transcription attempt can proceed
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

    // Give cpal stream a moment to flush its last callback
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    let (samples, captured_rate) = {
        let buf = state.audio_buffer.lock().unwrap();
        let rate = *state.native_sample_rate.lock().unwrap();
        (buf.clone(), rate)
    };

    if samples.is_empty() {
        return Ok(false);
    }

    let provider = SysinfoProvider::new();
    let profile = detect_profile(&provider);
    let override_size = { *state.model_override.lock().await };
    let selection = select_model(&profile, override_size);
    let model_filename = model_size_to_filename(selection.size);
    let model_path = state.models_dir.join(model_filename);
    let pool = state.pool.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        // Download model if missing
        if !model_path.exists() {
            let _ = app_handle.emit("model-download-start", ());
            if let Err(e) = download_whisper_model(&model_path, &app_handle) {
                let _ = app_handle.emit("transcription-error", format!("model download failed: {e}"));
                return;
            }
            let _ = app_handle.emit("model-download-complete", ());
        }

        // Resample from native device rate to 16kHz mono (required by Whisper)
        let resampled = resample_to_16k(&samples, captured_rate);

        let exec_provider = match profile.execution_provider.as_str() {
            "cuda"     => ExecutionProvider::Cuda,
            "directml" => ExecutionProvider::DirectML,
            _          => ExecutionProvider::Cpu,
        };

        match crate::inference::WhisperEngine::new(&model_path, exec_provider) {
            Ok(engine) => match engine.transcribe(&resampled) {
                Ok(text) if !text.is_empty() => {
                    let _ = app_handle.emit("transcription-complete", text.clone());
                    // Save transcript to DB
                    let repo = crate::database::repositories::transcript::TranscriptRepository::new(pool.clone());
                    let _ = tauri::async_runtime::block_on(
                        repo.create(crate::database::dto::transcript::CreateTranscript { content: text.clone() })
                    );
                    // Auto-learn: record uncommon words from this transcription
                    let freq_repo = crate::database::repositories::word_frequency::WordFrequencyRepository::new(pool);
                    let unknown = extract_unknown_words(&text);
                    if !unknown.is_empty() {
                        let _ = tauri::async_runtime::block_on(freq_repo.record_words(&unknown));
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    let _ = app_handle.emit("transcription-error", e);
                }
            },
            Err(e) => {
                let _ = app_handle.emit("transcription-error", e);
            }
        }
    });

    Ok(true)
}

fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    const TARGET: u32 = 16_000;
    if source_rate == TARGET || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = TARGET as f64 / source_rate as f64;
    let out_len = ((samples.len() as f64) * ratio).round().max(1.0) as usize;
    let max_idx = samples.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = (i as f64) / ratio;
        let left = src.floor() as usize;
        let right = (left + 1).min(max_idx);
        let frac = (src - left as f64) as f32;
        out.push(samples[left] * (1.0 - frac) + samples[right] * frac);
    }
    out
}

fn capture_microphone(
    running: Arc<std::sync::atomic::AtomicBool>,
    buffer: Arc<std::sync::Mutex<Vec<f32>>>,
    native_rate: Arc<std::sync::Mutex<u32>>,
) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleFormat;

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no input device available".to_string())?;

    let config = device
        .default_input_config()
        .map_err(|e| format!("failed to get input config: {e}"))?;

    let channels = config.channels() as usize;
    let sample_rate = config.sample_rate();
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    // Store the native rate so stop_transcription can resample correctly
    *native_rate.lock().unwrap() = sample_rate;

    let buf_clone = Arc::clone(&buffer);
    let running_clone = Arc::clone(&running);

    // Build and run the stream; data_callback converts to f32 mono
    let stream = match sample_format {
        SampleFormat::F32 => build_input_stream::<f32>(&device, &stream_config, channels, buf_clone, running_clone)?,
        SampleFormat::I16 => build_input_stream::<i16>(&device, &stream_config, channels, buf_clone, running_clone)?,
        SampleFormat::U16 => build_input_stream::<u16>(&device, &stream_config, channels, buf_clone, running_clone)?,
        _ => return Err(format!("unsupported sample format: {sample_format:?}")),
    };

    stream.play().map_err(|e| format!("failed to start stream: {e}"))?;

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    drop(stream);
    Ok(())
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    buffer: Arc<std::sync::Mutex<Vec<f32>>>,
    running: Arc<std::sync::atomic::AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + ToF32,
{
    use cpal::traits::DeviceTrait;

    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                // Convert to f32 mono
                let mono: Vec<f32> = if channels == 1 {
                    data.iter().map(|s| s.to_f32()).collect()
                } else {
                    data.chunks(channels)
                        .map(|frame| {
                            frame.iter().map(|s| s.to_f32()).sum::<f32>() / channels as f32
                        })
                        .collect()
                };
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend_from_slice(&mono);
                }
            },
            |err| eprintln!("cpal stream error: {err}"),
            None,
        )
        .map_err(|e| format!("failed to build input stream: {e}"))?;

    Ok(stream)
}

/// Conversion helper for cpal sample types to f32.
pub trait ToF32 {
    fn to_f32(self) -> f32;
}

impl ToF32 for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl ToF32 for i16 {
    fn to_f32(self) -> f32 {
        self as f32 / i16::MAX as f32
    }
}

impl ToF32 for u16 {
    fn to_f32(self) -> f32 {
        (self as f32 / u16::MAX as f32) * 2.0 - 1.0
    }
}

fn model_size_to_filename(size: ModelSize) -> &'static str {
    match size {
        ModelSize::Tiny   => "ggml-tiny.bin",
        ModelSize::Base   => "ggml-base.bin",
        ModelSize::Small  => "ggml-small.bin",
        ModelSize::Medium => "ggml-medium.bin",
        ModelSize::Large  => "ggml-large-v3.bin",
    }
}

fn download_whisper_model(dest: &std::path::Path, app: &AppHandle) -> Result<(), String> {
    use std::io::Write;

    let filename = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("ggml-tiny.bin");
    let model_url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{filename}"
    );

    let response = reqwest::blocking::get(&model_url)
        .map_err(|e| format!("download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("download HTTP {}", response.status()));
    }

    let total = response.content_length().unwrap_or(0);
    let bytes = response.bytes().map_err(|e| format!("read body failed: {e}"))?;

    if total > 0 {
        let _ = app.emit("model-download-progress", 100u8);
    }

    let mut file =
        std::fs::File::create(dest).map_err(|e| format!("create model file failed: {e}"))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write model file failed: {e}"))?;

    Ok(())
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
    // Rough estimate: average speaking pace ~130 wpm → words × (60/130) seconds
    let speaking_time_seconds = (total_words as f64 * 60.0 / 130.0).round() as i64;
    let avg_pace_wpm = if total_sessions > 0 { total_words / total_sessions.max(1) } else { 0 };

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

#[tauri::command]
pub async fn delete_dictionary_entry(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, ApiError> {
    let repo = DictionaryRepository::new(state.pool.clone());
    Ok(repo.delete_by_id(id).await?)
}

#[tauri::command]
pub async fn apply_dictionary(
    state: State<'_, AppState>,
    text: String,
) -> Result<String, ApiError> {
    let repo = DictionaryRepository::new(state.pool.clone());
    let engine = DictionaryCorrectionEngine::new(repo, DictionaryCorrectionConfig::default());
    engine.apply_to_text(&text).await.map_err(ApiError::from)
}

fn parse_model_size(value: &str) -> Result<ModelSize, ApiError> {
    match value.to_lowercase().as_str() {
        "tiny"   => Ok(ModelSize::Tiny),
        "base"   => Ok(ModelSize::Base),
        "small"  => Ok(ModelSize::Small),
        "medium" => Ok(ModelSize::Medium),
        "large"  => Ok(ModelSize::Large),
        _ => Err(ApiError::new("invalid_input", "invalid model size")),
    }
}

fn model_size_label(size: ModelSize) -> &'static str {
    match size {
        ModelSize::Tiny   => "tiny",
        ModelSize::Base   => "base",
        ModelSize::Small  => "small",
        ModelSize::Medium => "medium",
        ModelSize::Large  => "large",
    }
}

/// Extract words from transcription text that are likely domain-specific/uncommon.
/// Filters out: very short words, numbers, common English words.
fn extract_unknown_words(text: &str) -> Vec<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase())
        .filter(|w| {
            w.len() >= 4
                && w.chars().all(|c| c.is_alphabetic())
                && !is_common_word(w)
        })
        .collect();

    // Deduplicate within this transcription
    let mut seen = std::collections::HashSet::new();
    words.into_iter().filter(|w| seen.insert(w.clone())).collect()
}

/// Returns true if the word is a very common English word that should not be auto-learned.
/// This is a curated top-~300 list — enough to avoid noise without being exhaustive.
fn is_common_word(word: &str) -> bool {
    const COMMON: &[&str] = &[
        "that","this","with","from","have","been","were","they","their","them",
        "will","would","could","should","shall","there","where","when","what",
        "which","then","than","also","some","more","most","other","into","over",
        "just","because","after","about","before","between","through","during",
        "each","your","more","very","even","back","well","such","time","year",
        "know","think","make","take","come","look","want","give","find","tell",
        "work","call","need","feel","keep","last","long","much","many","down",
        "does","doing","done","made","said","went","come","came","goes","going",
        "here","only","same","again","still","around","every","right","small",
        "large","under","never","place","point","world","always","state","often",
        "those","these","thing","things","people","really","being","while","since",
        "both","help","must","high","next","part","home","hand","play","move",
        "live","hold","away","turn","show","open","seem","together","course",
        "nothing","something","everything","anything","someone","everyone","anyone",
        "actually","probably","already","without","however","because","whether",
        "another","getting","started","having","using","going","being","doing",
        "first","second","third","great","good","best","better","little","might",
        "almost","though","until","along","while","above","below","left","right",
    ];
    COMMON.contains(&word)
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordFrequencyResponse {
    pub word: String,
    pub count: i64,
}

/// Return words that appear frequently enough to be worth adding to the dictionary.
#[tauri::command]
pub async fn get_word_suggestions(
    state: State<'_, AppState>,
) -> Result<Vec<WordFrequencyResponse>, ApiError> {
    let repo = crate::database::repositories::word_frequency::WordFrequencyRepository::new(state.pool.clone());
    let entries = repo.unreviewed_above(3).await?;
    Ok(entries.into_iter().map(|e| WordFrequencyResponse { word: e.word, count: e.count }).collect())
}

/// Accept a suggestion — adds it to the dictionary and marks it reviewed.
#[tauri::command]
pub async fn accept_word_suggestion(
    state: State<'_, AppState>,
    word: String,
) -> Result<(), ApiError> {
    let freq_repo = crate::database::repositories::word_frequency::WordFrequencyRepository::new(state.pool.clone());
    let dict_repo = crate::database::repositories::dictionary::DictionaryRepository::new(state.pool.clone());
    dict_repo.upsert(crate::database::dto::dictionary::CreateDictionaryEntry {
        term: word.clone(),
        replacement: word.clone(),
    }).await?;
    freq_repo.mark_reviewed(&word, true).await?;
    Ok(())
}

/// Dismiss a suggestion — marks it reviewed without adding to dictionary.
#[tauri::command]
pub async fn dismiss_word_suggestion(
    state: State<'_, AppState>,
    word: String,
) -> Result<(), ApiError> {
    let repo = crate::database::repositories::word_frequency::WordFrequencyRepository::new(state.pool.clone());
    repo.mark_reviewed(&word, false).await?;
    Ok(())
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
pub async fn type_text(app: AppHandle, text: String) -> Result<(), ApiError> {
    use enigo::{Enigo, Key, Keyboard, Settings};
    use tauri_plugin_clipboard_manager::ClipboardExt;

    // Best-practice for voice-to-text on Windows: write to clipboard then
    // simulate Ctrl+V. Direct enigo.text() is unreliable — it synthesises
    // individual keystrokes and frequently drops characters or stops after
    // the first word due to Windows key-event rate limits.
    app.clipboard()
        .write_text(text)
        .map_err(|e| ApiError::new("clipboard_error", e.to_string()))?;

    std::thread::spawn(move || {
        // Small delay so the target window can regain focus after the pill
        // overlay releases its hold on input.
        std::thread::sleep(std::time::Duration::from_millis(150));
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            let _ = enigo.key(Key::Control, enigo::Direction::Press);
            let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
            let _ = enigo.key(Key::Control, enigo::Direction::Release);
        }
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

    // Validate input
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err(ApiError::new("hotkey_invalid", "hotkey cannot be empty"));
    }
    // Must contain at least one modifier + one non-modifier key
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

    // Unregister all existing shortcuts first to avoid duplicate registration
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| ApiError::new("hotkey_unregister_failed", e.to_string()))?;

    // Register with press/release handler
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
            // Map common OS errors to structured codes
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

    // Persist to disk and update in-memory state
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
    let mut current = state.current_hotkey.lock().await;
    *current = None;

    Ok(())
}

#[tauri::command]
pub async fn get_registered_hotkeys(state: State<'_, AppState>) -> Result<Vec<String>, ApiError> {
    let current = state.current_hotkey.lock().await;
    if let Some(h) = current.as_ref() {
        return Ok(vec![h.clone()]);
    }
    // Fallback: read from disk in case in-memory state hasn't been populated yet
    Ok(state.load_hotkey().map(|h| vec![h]).unwrap_or_default())
}
