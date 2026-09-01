#[derive(Debug, Clone)]
pub struct CreateTranscript {
    pub content: String,
    /// Word count pre-computed before insert so stats queries are O(1).
    pub word_count: i64,
    /// Actual recording duration in seconds derived from captured audio samples.
    pub duration_seconds: Option<f64>,
    /// Display name of the app that had focus when recording started.
    pub target_app: Option<String>,
}
