use sqlx::SqlitePool;

use crate::database::dto::dictionary::CreateDictionaryEntry;
use crate::database::models::dictionary::DictionaryEntry;

#[derive(Clone)]
pub struct DictionaryRepository {
    pool: SqlitePool,
}

impl DictionaryRepository {
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    #[cfg(test)]
    pub async fn create(
        &self,
        input: CreateDictionaryEntry,
    ) -> Result<DictionaryEntry, sqlx::Error> {
        sqlx::query_as::<_, DictionaryEntry>(
            "INSERT INTO dictionary (term, replacement) VALUES (?, ?)
             RETURNING id, term, replacement, hits, created_at",
        )
        .bind(input.term)
        .bind(input.replacement)
        .fetch_one(&self.pool)
        .await
    }

    #[cfg(test)]
    pub async fn get_by_term(&self, term: &str) -> Result<Option<DictionaryEntry>, sqlx::Error> {
        sqlx::query_as::<_, DictionaryEntry>(
            "SELECT id, term, replacement, hits, created_at FROM dictionary WHERE term = ?",
        )
        .bind(term)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_all(&self) -> Result<Vec<DictionaryEntry>, sqlx::Error> {
        sqlx::query_as::<_, DictionaryEntry>(
            "SELECT id, term, replacement, hits, created_at FROM dictionary",
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
             RETURNING id, term, replacement, hits, created_at",
        )
        .bind(input.term)
        .bind(input.replacement)
        .fetch_one(&self.pool)
        .await
    }

    /// Increment hit counter for each matched term in a single transaction.
    /// Counts how many times each term appears in the slice (one transcription
    /// could match the same word multiple times), then issues one UPDATE per
    /// distinct term — all inside one BEGIN/COMMIT so it is a single fsync.
    pub async fn increment_hits_batch(&self, terms: &[String]) -> Result<(), sqlx::Error> {
        if terms.is_empty() {
            return Ok(());
        }

        // Tally counts per distinct term (avoids duplicate UPDATE rows)
        let mut counts: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
        for t in terms {
            *counts.entry(t.as_str()).or_insert(0) += 1;
        }

        // Build:  WITH hits(term, n) AS (VALUES (?,?), (?,?), ...)
        //         UPDATE dictionary SET hits = hits + hits.n
        //         FROM hits WHERE dictionary.term = hits.term
        //
        // One statement, one lock, one fsync — O(distinct_terms) bind params.
        let placeholders: String = counts
            .keys()
            .map(|_| "(?,?)")
            .collect::<Vec<_>>()
            .join(",");

        let sql = format!(
            "WITH matched(term, n) AS (VALUES {placeholders}) \
             UPDATE dictionary SET hits = hits + matched.n \
             FROM matched WHERE dictionary.term = matched.term"
        );

        let mut q = sqlx::query(&sql);
        for (term, count) in &counts {
            q = q.bind(*term).bind(count);
        }

        let mut tx = self.pool.begin().await?;
        q.execute(&mut *tx).await?;
        tx.commit().await?;

        Ok(())
    }
}
