use sqlx::SqlitePool;

use crate::data::dto::transcript::CreateTranscript;
use crate::data::rows::transcript::{PendingImport, Transcript};
use crate::sync::{announce, Topic};

/// Every write to the transcripts table ends here: each is one statement, so
/// by now it has committed and a read the announcement triggers sees it.
fn announce_if(changed: bool) -> bool {
    if changed {
        announce(Topic::History);
    }
    changed
}

/// Last row of a page — identifies a row, not an index.
///
/// `created_at` must be `SQLite`'s `YYYY-MM-DD HH:MM:SS` text form, which is what
/// [`TranscriptResponse`](crate::bridge::commands::dto::TranscriptResponse) emits, so a
/// cursor echoed back by the frontend compares lexicographically against the column.
#[derive(Debug, Clone, Copy)]
pub struct Cursor<'a> {
    pub created_at: &'a str,
    pub id: i64,
}

/// A row value, not the spelled-out `a < ? OR (a = ? AND b < ?)`: SQLite seeks
/// the keyset index on a row value but scans it on the OR form.
fn keyset_sql(sort_desc: bool, has_cursor: bool) -> &'static str {
    match (sort_desc, has_cursor) {
        (true, true) => {
            "SELECT id, content, word_count, duration_seconds, target_app, pinned, has_audio, source_name, model, pending, revision, created_at
             FROM transcripts
             WHERE (? IS NULL OR created_at >= ?)
               AND (? IS NULL OR created_at <= ?)
               AND (created_at, id) < (?, ?)
             ORDER BY created_at DESC, id DESC
             LIMIT ?"
        }
        (false, true) => {
            "SELECT id, content, word_count, duration_seconds, target_app, pinned, has_audio, source_name, model, pending, revision, created_at
             FROM transcripts
             WHERE (? IS NULL OR created_at >= ?)
               AND (? IS NULL OR created_at <= ?)
               AND (created_at, id) > (?, ?)
             ORDER BY created_at ASC, id ASC
             LIMIT ?"
        }
        (true, false) => {
            "SELECT id, content, word_count, duration_seconds, target_app, pinned, has_audio, source_name, model, pending, revision, created_at
             FROM transcripts
             WHERE (? IS NULL OR created_at >= ?)
               AND (? IS NULL OR created_at <= ?)
             ORDER BY created_at DESC, id DESC
             LIMIT ?"
        }
        (false, false) => {
            "SELECT id, content, word_count, duration_seconds, target_app, pinned, has_audio, source_name, model, pending, revision, created_at
             FROM transcripts
             WHERE (? IS NULL OR created_at >= ?)
               AND (? IS NULL OR created_at <= ?)
             ORDER BY created_at ASC, id ASC
             LIMIT ?"
        }
    }
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
        let created = sqlx::query_as::<_, Transcript>(
            "INSERT INTO transcripts
               (content, word_count, duration_seconds, target_app, source_name, model, pending, language)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id, content, word_count, duration_seconds, target_app, pinned, has_audio, source_name, model, pending, revision, created_at",
        )
        .bind(input.content)
        .bind(input.word_count)
        .bind(input.duration_seconds)
        .bind(input.target_app)
        .bind(input.source_name)
        .bind(input.model)
        .bind(input.pending)
        .bind(input.language)
        .fetch_one(&self.pool)
        .await?;
        announce_if(true);
        Ok(created)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<Transcript>, sqlx::Error> {
        sqlx::query_as::<_, Transcript>(
            "SELECT id, content, word_count, duration_seconds, target_app, pinned, has_audio, source_name, model, pending, revision, created_at FROM transcripts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Returns (`total_sessions`, `total_words`, `total_duration_seconds`) from `transcript_stats`.
    pub async fn get_stats(&self) -> Result<(i64, i64, f64), sqlx::Error> {
        sqlx::query_as("SELECT sessions, words, duration_seconds FROM transcript_stats")
            .fetch_one(&self.pool)
            .await
    }

    /// Words dictated per local day over the last `days` days (today included),
    /// oldest first; days with no dictation are absent. `created_at` is UTC, so
    /// `utc_offset_minutes` shifts it into the user's day, and the lower bound is
    /// their local midnight shifted back to UTC so it can seek the keyset index.
    pub async fn daily_words(
        &self,
        days: i64,
        utc_offset_minutes: i64,
    ) -> Result<Vec<(String, i64)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT date(created_at, ?1) AS day, SUM(word_count)
             FROM transcripts
             WHERE created_at >= datetime('now', ?1, 'start of day', ?2, ?3)
             GROUP BY day
             ORDER BY day",
        )
        .bind(format!("{utc_offset_minutes:+} minutes"))
        .bind(format!("{} days", 1 - days))
        .bind(format!("{:+} minutes", -utc_offset_minutes))
        .fetch_all(&self.pool)
        .await
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
        let mut q = sqlx::query_as::<_, Transcript>(keyset_sql(sort_desc, cursor.is_some()))
            .bind(from)
            .bind(from)
            .bind(to)
            .bind(to);
        if let Some(c) = cursor {
            q = q.bind(c.created_at).bind(c.id);
        }
        q.bind(limit).fetch_all(&self.pool).await
    }

    pub async fn delete_by_id(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM transcripts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(announce_if(result.rows_affected() > 0))
    }

    /// Replace a transcript's text and the model it now comes from; clears
    /// `pending`.
    pub async fn set_text(&self, id: i64, content: &str, model: &str) -> Result<bool, sqlx::Error> {
        let word_count = i64::try_from(content.split_whitespace().count()).unwrap_or(i64::MAX);
        let result = sqlx::query(
            "UPDATE transcripts
             SET content = ?, word_count = ?, model = ?, pending = 0, revision = revision + 1
             WHERE id = ?",
        )
        .bind(content)
        .bind(word_count)
        .bind(model)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(announce_if(result.rows_affected() > 0))
    }

    /// Put back a transcript's earlier text and model, only while it is still
    /// at `revision`: a later rewrite wins over an older undo.
    pub async fn restore_text(
        &self,
        id: i64,
        revision: i64,
        content: &str,
        model: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let word_count = i64::try_from(content.split_whitespace().count()).unwrap_or(i64::MAX);
        let result = sqlx::query(
            "UPDATE transcripts
             SET content = ?, word_count = ?, model = ?, revision = revision + 1
             WHERE id = ? AND revision = ?",
        )
        .bind(content)
        .bind(word_count)
        .bind(model)
        .bind(id)
        .bind(revision)
        .execute(&self.pool)
        .await?;
        Ok(announce_if(result.rows_affected() > 0))
    }

    pub async fn set_pinned(&self, id: i64, pinned: bool) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE transcripts SET pinned = ? WHERE id = ?")
            .bind(pinned)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(announce_if(result.rows_affected() > 0))
    }

    /// Record that `id`'s audio file exists, or no longer does.
    pub async fn set_has_audio(&self, id: i64, has_audio: bool) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE transcripts SET has_audio = ? WHERE id = ?")
            .bind(has_audio)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(announce_if(result.rows_affected() > 0))
    }

    pub async fn pending(&self) -> Result<Vec<PendingImport>, sqlx::Error> {
        sqlx::query_as(
            "SELECT id, source_name, model, language FROM transcripts WHERE pending = 1 ORDER BY id",
        )
            .fetch_all(&self.pool)
            .await
    }

    /// Entries flagged as having saved audio.
    pub async fn ids_with_audio(&self) -> Result<Vec<i64>, sqlx::Error> {
        let rows: Vec<(i64,)> = sqlx::query_as("SELECT id FROM transcripts WHERE has_audio = 1")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Clear and return the audio flags past the newest `keep` (all when
    /// `None`), skipping pinned entries and pending imports. One statement,
    /// so a pin landing meanwhile is respected.
    pub async fn release_audio_beyond(&self, keep: Option<u32>) -> Result<Vec<i64>, sqlx::Error> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "UPDATE transcripts SET has_audio = 0 WHERE id IN (
                 SELECT id FROM transcripts WHERE has_audio = 1 AND pinned = 0 AND pending = 0
                 ORDER BY created_at DESC, id DESC LIMIT -1 OFFSET ?
             ) RETURNING id",
        )
        .bind(keep.map_or(0, i64::from))
        .fetch_all(&self.pool)
        .await?;
        announce_if(!rows.is_empty());
        Ok(rows.into_iter().map(|(id,)| id).collect())
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
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.target_app, t.pinned, t.has_audio, t.source_name, t.model, t.pending, t.revision, t.created_at
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
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.target_app, t.pinned, t.has_audio, t.source_name, t.model, t.pending, t.revision, t.created_at
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
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.target_app, t.pinned, t.has_audio, t.source_name, t.model, t.pending, t.revision, t.created_at
                 FROM transcripts_fts
                 JOIN transcripts t ON transcripts_fts.rowid = t.id
                 WHERE transcripts_fts MATCH ?
                   AND (? IS NULL OR t.created_at >= ?)
                   AND (? IS NULL OR t.created_at <= ?)
                 ORDER BY transcripts_fts.rowid DESC
                 LIMIT ?"
            }
            (false, false) => {
                "SELECT t.id, t.content, t.word_count, t.duration_seconds, t.target_app, t.pinned, t.has_audio, t.source_name, t.model, t.pending, t.revision, t.created_at
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
#[path = "../../../tests/unit/data/repositories/transcript.rs"]
mod tests;
