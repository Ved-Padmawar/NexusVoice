-- Reindex a transcript only when its text changes: a pin or an audio flag
-- rewrote the same text into the search index on every update.
DROP TRIGGER IF EXISTS transcripts_au;

CREATE TRIGGER transcripts_au AFTER UPDATE OF content ON transcripts
WHEN old.content IS NOT new.content BEGIN
  INSERT INTO transcripts_fts(transcripts_fts, rowid, content) VALUES ('delete', old.id, old.content);
  INSERT INTO transcripts_fts(rowid, content) VALUES (new.id, new.content);
END;
