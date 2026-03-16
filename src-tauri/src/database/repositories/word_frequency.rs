use sqlx::SqlitePool;

#[derive(Clone)]
pub struct WordFrequencyRepository {
    pool: SqlitePool,
}

impl WordFrequencyRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Increment frequency counters for a batch of words in one transaction.
    /// Returns words that just crossed the auto-learn threshold (count == 3).
    pub async fn increment_batch(&self, words: &[String]) -> Result<Vec<String>, sqlx::Error> {
        const THRESHOLD: i64 = 5;

        if words.is_empty() {
            return Ok(vec![]);
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
        tx.commit().await?;

        // Find words that exactly hit the threshold this batch (count == THRESHOLD)
        let mut newly_learned = Vec::new();
        for word in words {
            let count: i64 = sqlx::query_scalar(
                "SELECT count FROM word_frequency WHERE word = ? AND dismissed = 0",
            )
            .bind(word)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);
            if count == THRESHOLD {
                newly_learned.push(word.clone());
            }
        }
        Ok(newly_learned)
    }

}
