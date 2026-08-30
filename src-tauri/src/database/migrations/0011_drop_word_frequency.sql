-- Drop word_frequency: it backed dictionary auto-learn, which was removed.
-- Nothing has written the table since, so the only reader (fuzzy search-term
-- expansion) always matched against an empty vocabulary.
DROP INDEX IF EXISTS idx_word_frequency_count;
DROP TABLE IF EXISTS word_frequency;
