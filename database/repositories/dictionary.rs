use sqlx::SqlitePool;

use crate::database::dto::dictionary::CreateDictionaryEntry;
use crate::database::models::dictionary::DictionaryEntry;

#[derive(Clone)]
pub struct DictionaryRepository {
  pool: SqlitePool,
}

impl DictionaryRepository {
  pub fn new(pool: SqlitePool) -> Self {
    Self { pool }
  }

  #[allow(dead_code)]
  pub async fn create(&self, input: CreateDictionaryEntry) -> Result<DictionaryEntry, sqlx::Error> {
    sqlx::query_as::<_, DictionaryEntry>(
      "INSERT INTO dictionary (term, replacement) VALUES (?, ?) RETURNING id, term, replacement, created_at",
    )
    .bind(input.term)
    .bind(input.replacement)
    .fetch_one(&self.pool)
    .await
  }

  #[allow(dead_code)]
  pub async fn get_by_id(&self, id: i64) -> Result<Option<DictionaryEntry>, sqlx::Error> {
    sqlx::query_as::<_, DictionaryEntry>(
      "SELECT id, term, replacement, created_at FROM dictionary WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await
  }

  #[allow(dead_code)]
  pub async fn get_by_term(&self, term: &str) -> Result<Option<DictionaryEntry>, sqlx::Error> {
    sqlx::query_as::<_, DictionaryEntry>(
      "SELECT id, term, replacement, created_at FROM dictionary WHERE term = ?",
    )
    .bind(term)
    .fetch_optional(&self.pool)
    .await
  }

  pub async fn list_all(&self) -> Result<Vec<DictionaryEntry>, sqlx::Error> {
    sqlx::query_as::<_, DictionaryEntry>(
      "SELECT id, term, replacement, created_at FROM dictionary",
    )
    .fetch_all(&self.pool)
    .await
  }

  #[allow(dead_code)]
  pub async fn list_candidates(
    &self,
    prefix: &str,
    limit: i64,
  ) -> Result<Vec<DictionaryEntry>, sqlx::Error> {
    let pattern = format!("{prefix}%");
    sqlx::query_as::<_, DictionaryEntry>(
      "SELECT id, term, replacement, created_at FROM dictionary WHERE term LIKE ? LIMIT ?",
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(&self.pool)
    .await
  }

  pub async fn upsert(
    &self,
    input: CreateDictionaryEntry,
  ) -> Result<DictionaryEntry, sqlx::Error> {
    sqlx::query_as::<_, DictionaryEntry>(
      "INSERT INTO dictionary (term, replacement)\n       VALUES (?, ?)\n       ON CONFLICT(term) DO UPDATE SET replacement = excluded.replacement\n       RETURNING id, term, replacement, created_at",
    )
    .bind(input.term)
    .bind(input.replacement)
    .fetch_one(&self.pool)
    .await
  }
}
