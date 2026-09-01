use std::path::Path;
use std::time::Duration;

use sqlx::{
    migrate::MigrateError,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};

pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(database_url.trim_start_matches("sqlite://"))
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5));

    SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(options)
        .await
}

/// Configure PRAGMAs and run migrations. Pure — no file system access.
pub async fn init_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA synchronous = NORMAL;")
        .execute(pool)
        .await?;

    adopt_legacy_history(pool).await?;

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

/// Replay the `ADD COLUMN`s that introduced them, reaching the squashed
/// schema without losing the rows.
async fn add_missing_columns(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for (table, column, definition) in LATE_COLUMNS {
        if column_names(pool, table).await?.iter().any(|c| c == column) {
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

/// Bring a database predating the migration squash up to the current schema and
/// re-stamp it, so `open_database` does not wipe it as inconsistent.
async fn adopt_legacy_history(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let has_history: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if has_history.is_none() {
        return Ok(()); // Fresh database; the migrator will stamp it.
    }

    let (legacy_rows,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM _sqlx_migrations WHERE version > 1")
            .fetch_one(pool)
            .await?;
    if legacy_rows == 0 {
        return Ok(());
    }

    let (schema_ready,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM sqlite_master
         WHERE type='table' AND name IN ('transcripts', 'dictionary', 'transcripts_fts')",
    )
    .fetch_one(pool)
    .await?;
    if schema_ready != 3 {
        return Ok(());
    }
    add_missing_columns(pool).await?;

    // From the migrator, so editing the schema can't leave a stale constant.
    let migrator = sqlx::migrate!("src/database/migrations");
    let Some(current) = migrator.iter().find(|m| m.version == 1) else {
        return Ok(());
    };

    log::info!("adopting pre-squash migration history ({legacy_rows} legacy rows)");

    let mut tx = pool.begin().await?;

    // A database predating the auth removal still carries these.
    // app_session references users(id), so it goes first.
    sqlx::query("DROP TABLE IF EXISTS app_session")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DROP TABLE IF EXISTS refresh_tokens")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DROP TABLE IF EXISTS word_frequency")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DROP INDEX IF EXISTS idx_users_email")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DROP TABLE IF EXISTS users")
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM _sqlx_migrations")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO _sqlx_migrations
           (version, description, installed_on, success, checksum, execution_time)
         VALUES (?, ?, CURRENT_TIMESTAMP, 1, ?, 0)",
    )
    .bind(current.version)
    .bind(current.description.as_ref())
    .bind(current.checksum.as_ref())
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Open the database at `db_path` and run migrations.
///
/// If the migration history is inconsistent (e.g. after a partial upgrade),
/// backs up the existing file to `<db_path>.bak`, deletes it, and starts
/// fresh. All migrations are replayed on the clean DB.
///
/// Returns the ready pool.
pub async fn open_database(db_path: &Path) -> Result<SqlitePool, String> {
    let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));

    let pool = create_pool(&db_url)
        .await
        .map_err(|e| format!("database init failed: {e}"))?;

    match init_database(&pool).await {
        Ok(()) => Ok(pool),
        Err(sqlx::Error::Migrate(ref migrate_err)) if is_inconsistent(migrate_err) => {
            log::warn!(
                "migration state inconsistent: {migrate_err} — backing up and recreating database"
            );
            pool.close().await;

            // Timestamped, so a second recovery cannot overwrite the first backup.
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let bak = db_path.with_extension(format!("db.{stamp}.bak"));
            if let Err(e) = std::fs::copy(db_path, &bak) {
                log::warn!("could not write backup to {}: {e}", bak.display());
            } else {
                log::info!("database backed up to {}", bak.display());
            }

            // Remove the corrupted DB and start fresh
            std::fs::remove_file(db_path)
                .map_err(|e| format!("could not remove corrupted database: {e}"))?;

            let fresh_pool = create_pool(&db_url)
                .await
                .map_err(|e| format!("database recreate failed: {e}"))?;

            init_database(&fresh_pool)
                .await
                .map_err(|e| format!("migrations failed on fresh database: {e}"))?;

            Ok(fresh_pool)
        }
        Err(e) => Err(format!("database migrations failed: {e}")),
    }
}

const fn is_inconsistent(err: &MigrateError) -> bool {
    matches!(
        err,
        MigrateError::VersionMissing(_) | MigrateError::VersionMismatch(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn adopts_a_pre_squash_history_without_losing_rows() {
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
        assert_eq!(versions, 1, "history collapses to the single migration");

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
        assert_eq!(versions, 1);
    }

    #[tokio::test]
    async fn an_older_shaped_database_is_repaired_then_adopted() {
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

        adopt_legacy_history(&pool).await.expect("adopt runs");

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
        assert_eq!(versions, 1);
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
    async fn a_fresh_database_is_left_to_the_migrator() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_database(&pool).await.expect("init succeeds");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(versions, 1);
    }
}
