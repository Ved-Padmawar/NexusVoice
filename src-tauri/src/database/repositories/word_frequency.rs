use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WordFrequencyRow {
    pub word: String,
    pub count: i64,
}

#[derive(Clone)]
pub struct WordFrequencyRepository {
    pool: SqlitePool,
}

impl WordFrequencyRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Increment frequency counters for a batch of words in one transaction.
    pub async fn increment_batch(&self, words: &[String]) -> Result<(), sqlx::Error> {
        if words.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for word in words {
            sqlx::query(
                "INSERT INTO word_frequency (word, count, dismissed)
                 VALUES (?, 1, 0)
                 ON CONFLICT(word) DO UPDATE SET count = count + 1",
            )
            .bind(word)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await
    }

    /// Return words with count >= min_count that are not dismissed.
    pub async fn get_suggestions(
        &self,
        min_count: i64,
    ) -> Result<Vec<WordFrequencyRow>, sqlx::Error> {
        sqlx::query_as::<_, WordFrequencyRow>(
            "SELECT word, count FROM word_frequency
             WHERE count >= ? AND dismissed = 0
             ORDER BY count DESC",
        )
        .bind(min_count)
        .fetch_all(&self.pool)
        .await
    }

    /// Mark a word as dismissed so it never surfaces again.
    pub async fn dismiss(&self, word: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO word_frequency (word, count, dismissed)
             VALUES (?, 0, 1)
             ON CONFLICT(word) DO UPDATE SET dismissed = 1",
        )
        .bind(word)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
