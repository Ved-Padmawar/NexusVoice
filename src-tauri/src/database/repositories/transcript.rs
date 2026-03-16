use sqlx::SqlitePool;

use crate::database::dto::transcript::CreateTranscript;
use crate::database::models::transcript::Transcript;

#[derive(Clone)]
pub struct TranscriptRepository {
    pool: SqlitePool,
}

impl TranscriptRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateTranscript) -> Result<Transcript, sqlx::Error> {
        sqlx::query_as::<_, Transcript>(
            "INSERT INTO transcripts (content, word_count, duration_seconds)
             VALUES (?, ?, ?)
             RETURNING id, content, word_count, duration_seconds, created_at",
        )
        .bind(input.content)
        .bind(input.word_count)
        .bind(input.duration_seconds)
        .fetch_one(&self.pool)
        .await
    }

    #[allow(dead_code)]
    pub async fn get_by_id(&self, id: i64) -> Result<Option<Transcript>, sqlx::Error> {
        sqlx::query_as::<_, Transcript>(
            "SELECT id, content, word_count, duration_seconds, created_at FROM transcripts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Returns (total_sessions, total_words, total_duration_seconds) via single aggregate query.
    pub async fn get_stats(&self) -> Result<(i64, i64, f64), sqlx::Error> {
        let row: (i64, i64, Option<f64>) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(word_count), 0), SUM(duration_seconds) FROM transcripts",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok((row.0, row.1, row.2.unwrap_or(0.0)))
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<Transcript>, sqlx::Error> {
        sqlx::query_as::<_, Transcript>(
            "SELECT id, content, word_count, duration_seconds, created_at FROM transcripts ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
}
