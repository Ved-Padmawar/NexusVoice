-- Tracks how often each word appears in transcriptions.
-- Words with count >= threshold and not in the common-words blocklist
-- are candidates for auto-adding to the dictionary.
CREATE TABLE IF NOT EXISTS word_frequency (
  word      TEXT NOT NULL PRIMARY KEY,
  count     INTEGER NOT NULL DEFAULT 1,
  -- NULL means not yet reviewed; TRUE means added to dictionary; FALSE means dismissed
  reviewed  INTEGER,
  updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
