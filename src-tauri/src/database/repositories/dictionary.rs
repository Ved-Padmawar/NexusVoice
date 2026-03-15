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

    #[cfg(test)]
    pub async fn create(
        &self,
        input: CreateDictionaryEntry,
    ) -> Result<DictionaryEntry, sqlx::Error> {
        sqlx::query_as::<_, DictionaryEntry>(
            "INSERT INTO dictionary (term, replacement) VALUES (?, ?)
             RETURNING id, term, replacement, created_at",
        )
        .bind(input.term)
        .bind(input.replacement)
        .fetch_one(&self.pool)
        .await
    }

    #[cfg(test)]
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


    pub async fn delete_by_id(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM dictionary WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn upsert(
        &self,
        input: CreateDictionaryEntry,
    ) -> Result<DictionaryEntry, sqlx::Error> {
        sqlx::query_as::<_, DictionaryEntry>(
            "INSERT INTO dictionary (term, replacement)
             VALUES (?, ?)
             ON CONFLICT(term) DO UPDATE SET replacement = excluded.replacement
             RETURNING id, term, replacement, created_at",
        )
        .bind(input.term)
        .bind(input.replacement)
        .fetch_one(&self.pool)
        .await
    }
}
