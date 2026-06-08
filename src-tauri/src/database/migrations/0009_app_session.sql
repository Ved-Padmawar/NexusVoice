-- Simplify auth to a local single-profile session: no JWT, no rotating refresh
-- tokens. A single-row table records the currently signed-in user; login writes
-- it, logout deletes it, startup reads it.
CREATE TABLE IF NOT EXISTS app_session (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
);

-- The token-based auth is gone; its table is now unused.
DROP TABLE IF EXISTS refresh_tokens;
