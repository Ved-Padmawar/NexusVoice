use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct WordFrequencyEntry {
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

    /// Increment count for each word, inserting if new.
    pub async fn record_words(&self, words: &[String]) -> Result<(), sqlx::Error> {
        for word in words {
            sqlx::query(
                "INSERT INTO word_frequency (word, count, updated_at)
                 VALUES (?, 1, CURRENT_TIMESTAMP)
                 ON CONFLICT(word) DO UPDATE SET
                   count = count + 1,
                   updated_at = CURRENT_TIMESTAMP",
            )
            .bind(word)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Return words seen >= `min_count` times that haven't been reviewed yet.
    pub async fn unreviewed_above(
        &self,
        min_count: i64,
    ) -> Result<Vec<WordFrequencyEntry>, sqlx::Error> {
        sqlx::query_as::<_, (String, i64)>(
            "SELECT word, count FROM word_frequency
             WHERE count >= ? AND reviewed IS NULL
             ORDER BY count DESC
             LIMIT 50",
        )
        .bind(min_count)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(|(word, count)| WordFrequencyEntry { word, count }).collect())
    }

    /// Mark a word as reviewed (added to dictionary = true, dismissed = false).
    pub async fn mark_reviewed(&self, word: &str, added: bool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE word_frequency SET reviewed = ? WHERE word = ?",
        )
        .bind(added as i64)
        .bind(word)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
