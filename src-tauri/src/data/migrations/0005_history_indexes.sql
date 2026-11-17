-- Each entry carries the rowid (= id), so both walk in `(created_at, id)` order.

-- Pages read columns the old index lacked, so its copy of every transcript's
-- text saved no table visit and only doubled each write.
DROP INDEX IF EXISTS idx_transcripts_keyset;
CREATE INDEX idx_transcripts_keyset ON transcripts (created_at);

-- History filtered to one app, and the list of apps.
CREATE INDEX IF NOT EXISTS idx_transcripts_app ON transcripts (target_app, created_at);
