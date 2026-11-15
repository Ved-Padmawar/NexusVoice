-- Count completed dictations and imports. Pending imports contribute only
-- when their transcription commits.
DROP TRIGGER IF EXISTS transcript_stats_ai;
DROP TRIGGER IF EXISTS transcript_stats_ad;
DROP TRIGGER IF EXISTS transcript_stats_au;

INSERT OR REPLACE INTO transcript_stats (id, sessions, words, duration_seconds)
SELECT 1, COUNT(*), COALESCE(SUM(word_count), 0), COALESCE(SUM(duration_seconds), 0)
FROM transcripts WHERE pending = 0;

CREATE TRIGGER transcript_stats_ai AFTER INSERT ON transcripts WHEN new.pending = 0 BEGIN
  UPDATE transcript_stats SET
    sessions = sessions + 1,
    words = words + new.word_count,
    duration_seconds = duration_seconds + COALESCE(new.duration_seconds, 0);
END;

CREATE TRIGGER transcript_stats_ad AFTER DELETE ON transcripts WHEN old.pending = 0 BEGIN
  UPDATE transcript_stats SET
    sessions = sessions - 1,
    words = words - old.word_count,
    duration_seconds = duration_seconds - COALESCE(old.duration_seconds, 0);
END;

CREATE TRIGGER transcript_stats_au AFTER UPDATE OF pending, word_count, duration_seconds ON transcripts
WHEN old.pending = 0 OR new.pending = 0 BEGIN
  UPDATE transcript_stats SET
    sessions = sessions - (old.pending = 0) + (new.pending = 0),
    words = words - (old.pending = 0) * old.word_count + (new.pending = 0) * new.word_count,
    duration_seconds = duration_seconds - (old.pending = 0) * COALESCE(old.duration_seconds, 0)
        + (new.pending = 0) * COALESCE(new.duration_seconds, 0);
END;
