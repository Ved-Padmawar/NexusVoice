-- Recreate FTS5 table with tokenchars="'" so contractions like "wouldn't"
-- are indexed and searched as single tokens instead of being split on apostrophe.

-- Drop old triggers first
DROP TRIGGER IF EXISTS transcripts_ai;
DROP TRIGGER IF EXISTS transcripts_ad;
DROP TRIGGER IF EXISTS transcripts_au;

-- Drop and recreate FTS table with apostrophe as a token character
DROP TABLE IF EXISTS transcripts_fts;

CREATE VIRTUAL TABLE transcripts_fts
USING fts5(content, content='transcripts', content_rowid='id',
           tokenize="porter unicode61 tokenchars ''''");

-- Re-populate from existing rows
INSERT INTO transcripts_fts(rowid, content)
SELECT id, content FROM transcripts;

-- Recreate sync triggers
CREATE TRIGGER transcripts_ai AFTER INSERT ON transcripts BEGIN
  INSERT INTO transcripts_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER transcripts_ad AFTER DELETE ON transcripts BEGIN
  INSERT INTO transcripts_fts(transcripts_fts, rowid, content) VALUES ('delete', old.id, old.content);
END;

CREATE TRIGGER transcripts_au AFTER UPDATE ON transcripts BEGIN
  INSERT INTO transcripts_fts(transcripts_fts, rowid, content) VALUES ('delete', old.id, old.content);
  INSERT INTO transcripts_fts(rowid, content) VALUES (new.id, new.content);
END;
