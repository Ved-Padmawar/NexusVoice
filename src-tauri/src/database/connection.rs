use std::path::Path;
use std::time::Duration;

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(database_url.trim_start_matches("sqlite://"))
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5))
        // Per-connection pragma: run once through the pool it reaches one
        // connection. sqlx already enables foreign_keys on each.
        .synchronous(SqliteSynchronous::Normal);

    SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(options)
        .await
}

/// Configure PRAGMAs and run migrations. Pure — no file system access.
pub async fn init_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Persisted in the database file, so one connection setting it is enough.
    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(pool)
        .await?;

    reconcile_ledger(pool).await?;

    sqlx::migrate!("src/database/migrations")
        .run(pool)
        .await
        .map_err(|err| sqlx::Error::Migrate(Box::new(err)))
}

/// Columns an older database may predate, with their original definitions.
const LATE_COLUMNS: &[(&str, &str, &str)] = &[
    ("transcripts", "duration_seconds", "REAL"),
    ("transcripts", "word_count", "INTEGER NOT NULL DEFAULT 0"),
    ("transcripts", "target_app", "TEXT"),
    ("dictionary", "hits", "INTEGER NOT NULL DEFAULT 0"),
];

async fn column_names(pool: &SqlitePool, table: &str) -> Result<Vec<String>, sqlx::Error> {
    // The table name is a literal from `LATE_COLUMNS`, never user input.
    let rows: Vec<(String,)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT name FROM pragma_table_info('{table}')"
    )))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(name,)| name).collect())
}

/// Replay the `ADD COLUMN`s that introduced them, which `CREATE TABLE IF NOT
/// EXISTS` would skip, reaching the squashed schema without losing the rows.
async fn add_missing_columns(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for (table, column, definition) in LATE_COLUMNS {
        let columns = column_names(pool, table).await?;
        // No table yet: the migrator creates it with every column.
        if columns.is_empty() || columns.iter().any(|c| c == column) {
            continue;
        }
        log::info!("adding missing column {table}.{column}");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        )))
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Make the ledger agree with this build's migrations. Every migration is
/// idempotent, so a ledger that disagrees (pre-squash history, line endings
/// that changed a checksum, an edited migration, a half-applied run) is
/// dropped and the migrator replays them over the existing schema. Rows are
/// never touched. This is the only upgrade path.
async fn reconcile_ledger(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    add_missing_columns(pool).await?;

    let has_ledger: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if has_ledger.is_none() {
        return Ok(()); // Fresh database; the migrator will stamp it.
    }

    let migrator = sqlx::migrate!("src/database/migrations");
    let applied: Vec<(i64, Vec<u8>, bool)> =
        sqlx::query_as("SELECT version, checksum, success FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?;
    let in_step = applied.iter().all(|(version, checksum, success)| {
        *success
            && migrator
                .iter()
                .any(|m| m.version == *version && m.checksum.as_ref() == checksum.as_slice())
    });
    if in_step {
        return Ok(());
    }

    log::info!("rebuilding the migration ledger ({} rows)", applied.len());

    let mut tx = pool.begin().await?;
    // A database predating the auth removal still carries these.
    // app_session references users(id), so it goes first.
    for drop in [
        "DROP TABLE IF EXISTS app_session",
        "DROP TABLE IF EXISTS refresh_tokens",
        "DROP TABLE IF EXISTS word_frequency",
        "DROP INDEX IF EXISTS idx_users_email",
        "DROP TABLE IF EXISTS users",
        "DELETE FROM _sqlx_migrations",
    ] {
        sqlx::query(drop).execute(&mut *tx).await?;
    }
    tx.commit().await
}

/// Open and migrate without ever replacing a user's history. An incompatible
/// ledger needs an explicit supported upgrade (or a newer application), not a
/// fresh empty database. Closing on error also checkpoints the WAL safely.
pub async fn open_database(db_path: &Path) -> Result<SqlitePool, String> {
    let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = create_pool(&db_url)
        .await
        .map_err(|e| format!("database init failed: {e}"))?;
    if let Err(e) = init_database(&pool).await {
        pool.close().await;
        return Err(format!(
            "database migrations failed; original history retained: {e}"
        ));
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migration_count() -> i64 {
        i64::try_from(sqlx::migrate!("src/database/migrations").iter().count()).unwrap()
    }

    #[test]
    fn migrations_are_checked_out_with_lf_endings() {
        for sql in [
            include_str!("migrations/0001_init.sql"),
            include_str!("migrations/0002_transcript_stats.sql"),
        ] {
            assert!(!sql.contains('\r'));
        }
    }

    /// As the pre-squash build left it: end schema, 13 recorded versions.
    async fn legacy_database() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!("migrations/0001_init.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL,
                password_hash TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE app_session (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        for version in 1..=13 {
            sqlx::query(
                "INSERT INTO _sqlx_migrations
                   (version, description, success, checksum, execution_time)
                 VALUES (?, 'legacy', 1, X'00', 0)",
            )
            .bind(version)
            .execute(&pool)
            .await
            .unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn rebuilds_a_pre_squash_history_without_losing_rows() {
        let pool = legacy_database().await;
        sqlx::query("INSERT INTO transcripts (content, word_count) VALUES ('keep me', 2)")
            .execute(&pool)
            .await
            .unwrap();

        init_database(&pool).await.expect("init succeeds");

        let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM transcripts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1, "the transcript must survive adoption");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            versions,
            migration_count(),
            "history collapses to the squashed set"
        );

        let (auth_tables,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM sqlite_master
             WHERE type='table' AND name IN ('users', 'app_session', 'refresh_tokens')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(auth_tables, 0, "auth tables must be dropped on adoption");
    }

    #[tokio::test]
    async fn a_second_open_is_a_no_op() {
        let pool = legacy_database().await;
        init_database(&pool).await.expect("first open");
        init_database(&pool).await.expect("second open");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(versions, migration_count());
    }

    #[tokio::test]
    async fn an_older_shaped_database_is_repaired_then_migrated() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE transcripts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE TABLE dictionary (id INTEGER PRIMARY KEY, term TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE VIRTUAL TABLE transcripts_fts USING fts5(content)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        for version in 1..=12 {
            sqlx::query(
                "INSERT INTO _sqlx_migrations
                   (version, description, success, checksum, execution_time)
                 VALUES (?, 'legacy', 1, X'00', 0)",
            )
            .bind(version)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query("INSERT INTO transcripts (content) VALUES ('keep me')")
            .execute(&pool)
            .await
            .unwrap();

        init_database(&pool).await.expect("init succeeds");

        let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM transcripts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1, "the row must survive the repair");

        // The column whose absence broke every read must now exist.
        sqlx::query("SELECT target_app, word_count, duration_seconds FROM transcripts")
            .fetch_all(&pool)
            .await
            .expect("late columns were added");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(versions, migration_count());
    }

    /// The real upgrade path: a database shaped by the previous release, opened
    /// by this build.
    #[tokio::test]
    async fn upgrading_from_the_previous_release_keeps_transcripts() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nexusvoice.db");
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));

        let pool = create_pool(&url).await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE transcripts (id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL,
               created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP), duration_seconds REAL,
               word_count INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE dictionary (id INTEGER PRIMARY KEY AUTOINCREMENT, term TEXT NOT NULL,
               replacement TEXT NOT NULL, created_at TEXT, hits INTEGER NOT NULL DEFAULT 0);
             CREATE VIRTUAL TABLE transcripts_fts USING fts5(content);
             CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT);
             CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL,
               installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL,
               checksum BLOB NOT NULL, execution_time BIGINT NOT NULL);",
        )
        .execute(&pool)
        .await
        .unwrap();
        for version in 1..=12 {
            sqlx::query(
                "INSERT INTO _sqlx_migrations
                   (version, description, success, checksum, execution_time)
                 VALUES (?, 'legacy', 1, X'00', 0)",
            )
            .bind(version)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query("INSERT INTO transcripts (content, word_count) VALUES ('keep me', 2)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let pool = open_database(&db_path).await.expect("opens without wiping");

        let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM transcripts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1, "the transcript must survive the upgrade");

        // The query that was failing in the UI.
        sqlx::query(
            "SELECT id, content, word_count, duration_seconds, target_app, created_at
             FROM transcripts",
        )
        .fetch_all(&pool)
        .await
        .expect("get_transcripts works");

        let (users,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM sqlite_master WHERE name='users'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(users, 0, "auth tables dropped");
    }

    #[tokio::test]
    async fn the_stats_migration_totals_the_existing_transcripts() {
        let pool = legacy_database().await;
        sqlx::query("INSERT INTO transcripts (content, word_count, duration_seconds) VALUES ('a', 3, 1.5), ('b', 2, NULL)")
            .execute(&pool)
            .await
            .unwrap();

        init_database(&pool).await.expect("init succeeds");

        let totals: (i64, i64, f64) =
            sqlx::query_as("SELECT sessions, words, duration_seconds FROM transcript_stats")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(totals, (2, 5, 1.5));
    }

    #[tokio::test]
    async fn the_stats_follow_inserts_updates_and_deletes() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_database(&pool).await.expect("init succeeds");

        sqlx::query("INSERT INTO transcripts (content, word_count, duration_seconds) VALUES ('a', 3, 2.0), ('b', 4, 1.0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE transcripts SET word_count = 10 WHERE content = 'a'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM transcripts WHERE content = 'b'")
            .execute(&pool)
            .await
            .unwrap();

        let totals: (i64, i64, f64) =
            sqlx::query_as("SELECT sessions, words, duration_seconds FROM transcript_stats")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(totals, (1, 10, 2.0));
    }

    #[tokio::test]
    async fn a_fresh_database_is_left_to_the_migrator() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_database(&pool).await.expect("init succeeds");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(versions, migration_count());
    }

    #[tokio::test]
    async fn reopening_a_migrated_database_keeps_its_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        open_database(&path).await.unwrap().close().await;

        let pool = open_database(&path).await.expect("reopen succeeds");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(versions, migration_count());
    }

    #[tokio::test]
    async fn a_failed_migration_never_replaces_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let pool = open_database(&path).await.unwrap();
        sqlx::query("INSERT INTO transcripts(content) VALUES ('sentinel')")
            .execute(&pool)
            .await
            .unwrap();
        // A ledger row this build has never seen, as a newer build leaves it.
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (999, 'future', 1, X'00', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let _ = open_database(&path).await;

        let pool = create_pool(&path.to_string_lossy()).await.unwrap();
        let (content,): (String,) = sqlx::query_as("SELECT content FROM transcripts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(content, "sentinel");
    }

    /// What a database looks like after a build ran every migration, with the
    /// ledger rows rewritten by `tamper`.
    async fn migrated_database_with_ledger(tamper: &str) -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_database(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcripts (content, word_count, duration_seconds) VALUES ('a', 3, 1.5), ('b', 2, NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(tamper.to_string()))
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    async fn assert_healthy(pool: &SqlitePool) {
        init_database(pool).await.expect("init succeeds");
        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(versions, migration_count());
        let totals: (i64, i64, f64) =
            sqlx::query_as("SELECT sessions, words, duration_seconds FROM transcript_stats")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(totals, (2, 5, 1.5));
        let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM transcripts")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(rows, 2, "no transcript is lost");
        // The triggers still work after the replay.
        sqlx::query("INSERT INTO transcripts (content, word_count) VALUES ('c', 1)")
            .execute(pool)
            .await
            .unwrap();
        let (sessions,): (i64,) = sqlx::query_as("SELECT sessions FROM transcript_stats")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(sessions, 3);
    }

    /// The failure that shipped: ledger checksums from a CRLF build, while the
    /// stats table already exists.
    #[tokio::test]
    async fn a_ledger_with_line_ending_checksums_is_rebuilt() {
        let pool =
            migrated_database_with_ledger("UPDATE _sqlx_migrations SET checksum = X'AA'").await;
        assert_healthy(&pool).await;
    }

    #[tokio::test]
    async fn a_ledger_missing_the_stats_migration_is_completed() {
        let pool =
            migrated_database_with_ledger("DELETE FROM _sqlx_migrations WHERE version = 2").await;
        assert_healthy(&pool).await;
    }

    #[tokio::test]
    async fn a_ledger_stamped_for_the_first_migration_only_is_completed() {
        // The state the earlier adoption left behind.
        let pool =
            migrated_database_with_ledger("DELETE FROM _sqlx_migrations WHERE version > 1").await;
        assert_healthy(&pool).await;
    }

    #[tokio::test]
    async fn an_edited_migration_is_replayed() {
        let pool = migrated_database_with_ledger(
            "UPDATE _sqlx_migrations SET checksum = X'BB' WHERE version = 2",
        )
        .await;
        assert_healthy(&pool).await;
    }

    #[tokio::test]
    async fn a_half_applied_ledger_row_is_replayed() {
        let pool = migrated_database_with_ledger(
            "UPDATE _sqlx_migrations SET success = 0 WHERE version = 2",
        )
        .await;
        assert_healthy(&pool).await;
    }

    #[tokio::test]
    async fn drifted_stats_are_recomputed_by_the_replay() {
        let pool = migrated_database_with_ledger(
            "UPDATE transcript_stats SET sessions = 99, words = 99; DELETE FROM _sqlx_migrations WHERE version = 2",
        )
        .await;
        assert_healthy(&pool).await;
    }

    #[tokio::test]
    async fn a_ledger_row_from_a_newer_build_is_rebuilt_without_losing_rows() {
        let pool = migrated_database_with_ledger(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (999, 'future', 1, X'00', 0)",
        )
        .await;
        assert_healthy(&pool).await;
    }

    #[tokio::test]
    async fn every_migration_can_run_twice() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for _ in 0..2 {
            for sql in [
                include_str!("migrations/0001_init.sql"),
                include_str!("migrations/0002_transcript_stats.sql"),
            ] {
                sqlx::raw_sql(sql).execute(&pool).await.unwrap();
            }
        }
    }

    #[tokio::test]
    async fn a_database_survives_many_reopens_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let pool = open_database(&path).await.unwrap();
        sqlx::query("INSERT INTO transcripts (content, word_count) VALUES ('keep', 1)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        for _ in 0..3 {
            let pool = open_database(&path).await.expect("reopen succeeds");
            let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM transcripts")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(rows, 1);
            pool.close().await;
        }
    }

    #[tokio::test]
    async fn a_healthy_ledger_is_left_untouched() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_database(&pool).await.unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET execution_time = 4242")
            .execute(&pool)
            .await
            .unwrap();

        init_database(&pool).await.unwrap();

        let (stamped,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM _sqlx_migrations WHERE execution_time = 4242")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stamped, migration_count(), "no row was replayed");
    }

    #[tokio::test]
    async fn transcripts_that_predate_the_search_index_become_searchable() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE transcripts (id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL,
               created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP), duration_seconds REAL,
               word_count INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE dictionary (id INTEGER PRIMARY KEY AUTOINCREMENT, term TEXT NOT NULL,
               replacement TEXT NOT NULL, created_at TEXT, hits INTEGER NOT NULL DEFAULT 0);
             INSERT INTO transcripts (content) VALUES ('an old dictation');",
        )
        .execute(&pool)
        .await
        .unwrap();

        init_database(&pool).await.expect("init succeeds");

        let hits: Vec<(i64,)> = sqlx::query_as(
            "SELECT rowid FROM transcripts_fts WHERE transcripts_fts MATCH 'dictation'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[tokio::test]
    async fn replaying_the_migrations_keeps_search_results_unchanged() {
        let pool = migrated_database_with_ledger("DELETE FROM _sqlx_migrations").await;
        init_database(&pool).await.expect("replay succeeds");

        let (hits,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM transcripts_fts WHERE transcripts_fts MATCH 'a OR b'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 2);
    }
}
