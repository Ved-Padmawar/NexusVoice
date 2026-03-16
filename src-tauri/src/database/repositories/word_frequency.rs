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
    /// Returns words that just crossed the auto-learn threshold (count == THRESHOLD).
    /// Uses a single batch SELECT instead of N per-word queries.
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

        // Single query to find all words that just hit the threshold — no N+1
        let placeholders = words.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let query = format!(
            "SELECT word FROM word_frequency WHERE word IN ({placeholders}) AND count = ? AND dismissed = 0"
        );
        let mut q = sqlx::query_scalar::<_, String>(&query);
        for word in words {
            q = q.bind(word);
        }
        q = q.bind(THRESHOLD);
        let newly_learned = q.fetch_all(&self.pool).await?;

        Ok(newly_learned)
    }
}
