-- Idempotent like every migration: the ledger can be rebuilt over this schema,
-- and the totals are recomputed rather than trusted.
-- Running totals, so the dashboard stats read one row instead of scanning every transcript.
CREATE TABLE IF NOT EXISTS transcript_stats (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  sessions INTEGER NOT NULL,
  words INTEGER NOT NULL,
  duration_seconds REAL NOT NULL
);

INSERT OR REPLACE INTO transcript_stats (id, sessions, words, duration_seconds)
SELECT 1, COUNT(*), COALESCE(SUM(word_count), 0), COALESCE(SUM(duration_seconds), 0)
FROM transcripts;

CREATE TRIGGER IF NOT EXISTS transcript_stats_ai AFTER INSERT ON transcripts BEGIN
  UPDATE transcript_stats SET
    sessions = sessions + 1,
    words = words + new.word_count,
    duration_seconds = duration_seconds + COALESCE(new.duration_seconds, 0);
END;

CREATE TRIGGER IF NOT EXISTS transcript_stats_ad AFTER DELETE ON transcripts BEGIN
  UPDATE transcript_stats SET
    sessions = sessions - 1,
    words = words - old.word_count,
    duration_seconds = duration_seconds - COALESCE(old.duration_seconds, 0);
END;

CREATE TRIGGER IF NOT EXISTS transcript_stats_au AFTER UPDATE OF word_count, duration_seconds ON transcripts BEGIN
  UPDATE transcript_stats SET
    words = words - old.word_count + new.word_count,
    duration_seconds = duration_seconds - COALESCE(old.duration_seconds, 0) + COALESCE(new.duration_seconds, 0);
END;
