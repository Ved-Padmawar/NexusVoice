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
      "INSERT INTO transcripts (content) VALUES (?) RETURNING id, content, created_at",
    )
    .bind(input.content)
    .fetch_one(&self.pool)
    .await
  }

  #[allow(dead_code)]
  pub async fn get_by_id(&self, id: i64) -> Result<Option<Transcript>, sqlx::Error> {
    sqlx::query_as::<_, Transcript>(
      "SELECT id, content, created_at FROM transcripts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await
  }

  pub async fn list_recent(&self, limit: i64) -> Result<Vec<Transcript>, sqlx::Error> {
    sqlx::query_as::<_, Transcript>(
      "SELECT id, content, created_at FROM transcripts ORDER BY created_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&self.pool)
    .await
  }
}
