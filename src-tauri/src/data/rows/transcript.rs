use chrono::NaiveDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Transcript {
    pub id: i64,
    pub content: String,
    pub word_count: i64,
    pub duration_seconds: Option<f64>,
    /// Display name of the app this was dictated into ("VS Code"). `None` when
    /// the foreground app could not be determined, and for pre-1.17 rows.
    pub target_app: Option<String>,
    pub pinned: bool,
    /// The audio file exists. Set only once it is on disk.
    pub has_audio: bool,
    /// File name of the imported audio; `None` for a dictation.
    pub source_name: Option<String>,
    /// Catalog id of the model that produced the text; `None` before 1.21.
    pub model: Option<String>,
    /// An import whose text is still being transcribed.
    pub pending: bool,
    /// Bumped by every rewrite of the text, so an undo can tell it is stale.
    pub revision: i64,
    pub created_at: NaiveDateTime,
}

/// An import still to be transcribed, with what its job was queued with.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingImport {
    pub id: i64,
    pub source_name: Option<String>,
    pub model: Option<String>,
    pub language: Option<String>,
}
