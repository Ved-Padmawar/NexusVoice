-- Keyset pagination orders by the (created_at, id) tuple: CURRENT_TIMESTAMP is only
-- second-granular, so id breaks ties. SQLite walks this index backwards for DESC, so
-- one ASC index serves both sort directions. Covering for the paged columns.
CREATE INDEX IF NOT EXISTS idx_transcripts_keyset
  ON transcripts (created_at, id, content, word_count, duration_seconds);

DROP INDEX IF EXISTS idx_transcripts_created_at;
