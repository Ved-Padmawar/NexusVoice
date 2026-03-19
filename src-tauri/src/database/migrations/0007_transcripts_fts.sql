-- FTS5 virtual table for full-text search on transcript content
CREATE VIRTUAL TABLE IF NOT EXISTS transcripts_fts
USING fts5(content, content='transcripts', content_rowid='id', tokenize='porter unicode61');

-- Populate FTS from existing rows
INSERT OR IGNORE INTO transcripts_fts(rowid, content)
SELECT id, content FROM transcripts;

-- Keep FTS in sync via triggers
CREATE TRIGGER IF NOT EXISTS transcripts_ai AFTER INSERT ON transcripts BEGIN
  INSERT INTO transcripts_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS transcripts_ad AFTER DELETE ON transcripts BEGIN
  INSERT INTO transcripts_fts(transcripts_fts, rowid, content) VALUES ('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS transcripts_au AFTER UPDATE ON transcripts BEGIN
  INSERT INTO transcripts_fts(transcripts_fts, rowid, content) VALUES ('delete', old.id, old.content);
  INSERT INTO transcripts_fts(rowid, content) VALUES (new.id, new.content);
END;
