-- The keyset index copied each transcript's text so a page could be read from
-- the index alone. Pages read columns it lacks, so they visit the table anyway
-- and the copy only doubled every write. Each entry carries the rowid (= id),
-- so `(created_at)` still orders by `(created_at, id)`.
DROP INDEX IF EXISTS idx_transcripts_keyset;

CREATE INDEX idx_transcripts_keyset ON transcripts (created_at);
