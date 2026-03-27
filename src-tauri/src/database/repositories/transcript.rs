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

    /// Returns (`total_sessions`, `total_words`, `total_duration_seconds`) via single aggregate query.
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
            "SELECT id, content, word_count, duration_seconds, created_at
             FROM transcripts ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Paginated fetch with optional date range and sort order.
    pub async fn list_paginated(
        &self,
        limit: i64,
        offset: i64,
        from: Option<&str>,
        to: Option<&str>,
        sort_desc: bool,
    ) -> Result<Vec<Transcript>, sqlx::Error> {
        let sql = if sort_desc {
            "SELECT id, content, word_count, duration_seconds, created_at
             FROM transcripts
             WHERE (? IS NULL OR created_at >= ?)
               AND (? IS NULL OR created_at <= ?)
             ORDER BY created_at DESC
             LIMIT ? OFFSET ?"
        } else {
            "SELECT id, content, word_count, duration_seconds, created_at
             FROM transcripts
             WHERE (? IS NULL OR created_at >= ?)
               AND (? IS NULL OR created_at <= ?)
             ORDER BY created_at ASC
             LIMIT ? OFFSET ?"
        };
        sqlx::query_as::<_, Transcript>(sql)
            .bind(from).bind(from)
            .bind(to).bind(to)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
    }

    /// Returns all transcripts ordered by date — used for export.
    pub async fn list_all(&self) -> Result<Vec<Transcript>, sqlx::Error> {
        sqlx::query_as::<_, Transcript>(
            "SELECT id, content, word_count, duration_seconds, created_at
             FROM transcripts ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Transforms a user query into an FTS5 query string.
    /// For each query word:
    ///   - Always includes the word itself and a prefix variant (word*)
    ///   - Uses `strsim::jaro_winkler` to find close matches from the user's vocabulary
    ///     (score >= 0.88 and not identical) and adds them as OR alternatives
    pub fn build_fts_query(query: &str, vocab: &[String]) -> String {
        let fts_terms: Vec<String> = query
            .split_whitespace()
            .map(|w| {
                let w_lower = w.to_lowercase();
                let mut variants: Vec<String> = vec![w_lower.clone()];

                // Prefix match for partial typing
                if w_lower.len() >= 3 {
                    variants.push(format!("{w_lower}*"));
                }

                // Fuzzy matches from user vocabulary via Jaro-Winkler
                if w_lower.len() >= 4 {
                    for candidate in vocab {
                        let c_lower = candidate.to_lowercase();
                        if c_lower == w_lower {
                            continue;
                        }
                        let score = strsim::jaro_winkler(&w_lower, &c_lower);
                        if score >= 0.88 {
                            variants.push(c_lower);
                        }
                    }
                }

                variants.join(" OR ")
            })
            .collect();

        fts_terms.join(" OR ")
    }

    /// FTS5 search with optional date range and sort order.
    pub async fn search(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
        from: Option<&str>,
        to: Option<&str>,
        sort_desc: bool,
    ) -> Result<Vec<Transcript>, sqlx::Error> {
        let sql = if sort_desc {
            "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.created_at
             FROM transcripts_fts
             JOIN transcripts t ON transcripts_fts.rowid = t.id
             WHERE transcripts_fts MATCH ?
               AND (? IS NULL OR t.created_at >= ?)
               AND (? IS NULL OR t.created_at <= ?)
             ORDER BY t.created_at DESC
             LIMIT ? OFFSET ?"
        } else {
            "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.created_at
             FROM transcripts_fts
             JOIN transcripts t ON transcripts_fts.rowid = t.id
             WHERE transcripts_fts MATCH ?
               AND (? IS NULL OR t.created_at >= ?)
               AND (? IS NULL OR t.created_at <= ?)
             ORDER BY t.created_at ASC
             LIMIT ? OFFSET ?"
        };
        sqlx::query_as::<_, Transcript>(sql)
            .bind(query)
            .bind(from).bind(from)
            .bind(to).bind(to)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
    }
}
