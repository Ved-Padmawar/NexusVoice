use sqlx::SqlitePool;

use crate::database::dto::transcript::CreateTranscript;
use crate::database::models::transcript::Transcript;

/// Last row of a page — identifies a row, not an index.
///
/// `created_at` must be `SQLite`'s `YYYY-MM-DD HH:MM:SS` text form, which is what
/// [`TranscriptResponse`](crate::commands::dto::TranscriptResponse) emits, so a
/// cursor echoed back by the frontend compares lexicographically against the column.
#[derive(Debug, Clone, Copy)]
pub struct Cursor<'a> {
    pub created_at: &'a str,
    pub id: i64,
}

#[derive(Clone)]
pub struct TranscriptRepository {
    pool: SqlitePool,
}

impl TranscriptRepository {
    pub const fn new(pool: SqlitePool) -> Self {
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

    /// Keyset-paginated fetch: returns rows strictly after `cursor`, ordered by the
    /// `(created_at, id)` tuple. Unlike LIMIT/OFFSET, a cursor names a row, so an
    /// insert or delete mid-scroll can't shift the window into repeats or skips.
    pub async fn list_keyset(
        &self,
        limit: i64,
        cursor: Option<Cursor<'_>>,
        from: Option<&str>,
        to: Option<&str>,
        sort_desc: bool,
    ) -> Result<Vec<Transcript>, sqlx::Error> {
        // Tuple comparison spelled out rather than a row value: past created_at, or
        // the same second with an id past the cursor's.
        let sql = match (sort_desc, cursor.is_some()) {
            (true, true) => {
                "SELECT id, content, word_count, duration_seconds, created_at
                 FROM transcripts
                 WHERE (? IS NULL OR created_at >= ?)
                   AND (? IS NULL OR created_at <= ?)
                   AND (created_at < ? OR (created_at = ? AND id < ?))
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?"
            }
            (false, true) => {
                "SELECT id, content, word_count, duration_seconds, created_at
                 FROM transcripts
                 WHERE (? IS NULL OR created_at >= ?)
                   AND (? IS NULL OR created_at <= ?)
                   AND (created_at > ? OR (created_at = ? AND id > ?))
                 ORDER BY created_at ASC, id ASC
                 LIMIT ?"
            }
            (true, false) => {
                "SELECT id, content, word_count, duration_seconds, created_at
                 FROM transcripts
                 WHERE (? IS NULL OR created_at >= ?)
                   AND (? IS NULL OR created_at <= ?)
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?"
            }
            (false, false) => {
                "SELECT id, content, word_count, duration_seconds, created_at
                 FROM transcripts
                 WHERE (? IS NULL OR created_at >= ?)
                   AND (? IS NULL OR created_at <= ?)
                 ORDER BY created_at ASC, id ASC
                 LIMIT ?"
            }
        };

        let mut q = sqlx::query_as::<_, Transcript>(sql)
            .bind(from)
            .bind(from)
            .bind(to)
            .bind(to);
        if let Some(c) = cursor {
            q = q.bind(c.created_at).bind(c.created_at).bind(c.id);
        }
        q.bind(limit).fetch_all(&self.pool).await
    }

    pub async fn delete_by_id(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM transcripts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
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

    /// Transforms a user query into an FTS5 query string: each word plus a
    /// prefix variant (`word*`), OR-ed together.
    ///
    /// Every term is emitted as a **quoted FTS5 string literal** so user input
    /// containing FTS syntax characters (`"`, `-`, `:`, `*`, `(`, `^`, etc.) is
    /// treated as literal text instead of breaking the MATCH grammar.
    pub fn build_fts_query(query: &str) -> String {
        // Wrap a token as a quoted FTS5 literal, doubling embedded quotes.
        fn quote(token: &str) -> String {
            format!("\"{}\"", token.replace('"', "\"\""))
        }

        let fts_terms: Vec<String> = query
            .split_whitespace()
            .filter_map(|w| {
                let w_lower = w.to_lowercase();
                // Skip tokens with no FTS-meaningful characters (e.g. lone punctuation),
                // which would otherwise produce an empty quoted literal.
                if !w_lower.chars().any(char::is_alphanumeric) {
                    return None;
                }
                let mut variants: Vec<String> = vec![quote(&w_lower)];

                // Prefix match for partial typing — `*` must sit OUTSIDE the quotes.
                if w_lower.len() >= 3 {
                    variants.push(format!("{}*", quote(&w_lower)));
                }

                Some(variants.join(" OR "))
            })
            .collect();

        fts_terms.join(" OR ")
    }

    /// FTS5 search with optional date range and sort order.
    ///
    /// Ordered by `rowid` (= `transcripts.id`), not by `created_at`. Ordering by
    /// `created_at` forces SQLite to materialize *every* match into a temp B-tree
    /// and sort it before `LIMIT` discards all but a page — cost scales with the
    /// number of matches, not the page size, so a common word costs ~70 ms per
    /// 50 k matched rows. Ordering by `rowid` lets FTS5 walk its own index and
    /// stop at `LIMIT` (~0.1 ms).
    ///
    /// This is equivalent **only because `created_at` is never written
    /// explicitly**: `create` omits the column, so it always takes
    /// `CURRENT_TIMESTAMP` and ids therefore rise with time. Backdating a row
    /// would break both this ordering and the cursor below.
    pub async fn search(
        &self,
        query: &str,
        limit: i64,
        cursor: Option<Cursor<'_>>,
        from: Option<&str>,
        to: Option<&str>,
        sort_desc: bool,
    ) -> Result<Vec<Transcript>, sqlx::Error> {
        let sql = match (sort_desc, cursor.is_some()) {
            (true, true) => {
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.created_at
                 FROM transcripts_fts
                 JOIN transcripts t ON transcripts_fts.rowid = t.id
                 WHERE transcripts_fts MATCH ?
                   AND (? IS NULL OR t.created_at >= ?)
                   AND (? IS NULL OR t.created_at <= ?)
                   AND transcripts_fts.rowid < ?
                 ORDER BY transcripts_fts.rowid DESC
                 LIMIT ?"
            }
            (false, true) => {
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.created_at
                 FROM transcripts_fts
                 JOIN transcripts t ON transcripts_fts.rowid = t.id
                 WHERE transcripts_fts MATCH ?
                   AND (? IS NULL OR t.created_at >= ?)
                   AND (? IS NULL OR t.created_at <= ?)
                   AND transcripts_fts.rowid > ?
                 ORDER BY transcripts_fts.rowid ASC
                 LIMIT ?"
            }
            (true, false) => {
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.created_at
                 FROM transcripts_fts
                 JOIN transcripts t ON transcripts_fts.rowid = t.id
                 WHERE transcripts_fts MATCH ?
                   AND (? IS NULL OR t.created_at >= ?)
                   AND (? IS NULL OR t.created_at <= ?)
                 ORDER BY transcripts_fts.rowid DESC
                 LIMIT ?"
            }
            (false, false) => {
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.created_at
                 FROM transcripts_fts
                 JOIN transcripts t ON transcripts_fts.rowid = t.id
                 WHERE transcripts_fts MATCH ?
                   AND (? IS NULL OR t.created_at >= ?)
                   AND (? IS NULL OR t.created_at <= ?)
                 ORDER BY transcripts_fts.rowid ASC
                 LIMIT ?"
            }
        };

        let mut q = sqlx::query_as::<_, Transcript>(sql)
            .bind(query)
            .bind(from)
            .bind(from)
            .bind(to)
            .bind(to);
        if let Some(c) = cursor {
            q = q.bind(c.id);
        }
        q.bind(limit).fetch_all(&self.pool).await
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/database/repositories/transcript.rs"]
mod tests;
