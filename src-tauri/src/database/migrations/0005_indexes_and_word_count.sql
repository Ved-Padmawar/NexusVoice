-- Add word_count to transcripts for O(1) stats aggregation
ALTER TABLE transcripts ADD COLUMN word_count INTEGER NOT NULL DEFAULT 0;

-- Index for word_frequency threshold queries
CREATE INDEX IF NOT EXISTS idx_word_frequency_count ON word_frequency (count, dismissed);

-- Composite index for token validation query (token_hash + revoked + expires_at)
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_lookup ON refresh_tokens (token_hash, revoked, expires_at);
