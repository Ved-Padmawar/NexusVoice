use std::path::Path;
use std::time::Duration;

use sqlx::{
    migrate::MigrateError,
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

    sqlx::migrate!("src/database/migrations")
        .run(pool)
        .await
        .map_err(|err| sqlx::Error::Migrate(Box::new(err)))
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

    fn migration_count() -> i64 {
        i64::try_from(sqlx::migrate!("src/database/migrations").iter().count()).unwrap()
    }

    async fn memory_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn the_stats_migration_totals_the_existing_transcripts() {
        let pool = memory_pool().await;
        sqlx::migrate!("src/database/migrations")
            .run_to(1, &pool)
            .await
            .expect("first migration applies");
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
        let pool = memory_pool().await;
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
        let pool = memory_pool().await;
        init_database(&pool).await.expect("init succeeds");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(versions, migration_count());
    }

    #[tokio::test]
    async fn a_second_init_is_a_no_op() {
        let pool = memory_pool().await;
        init_database(&pool).await.expect("first init");
        init_database(&pool).await.expect("second init");

        let (versions,): (i64,) = sqlx::query_as("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(versions, migration_count());
    }
}
