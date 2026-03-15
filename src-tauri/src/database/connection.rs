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
    let db_url = format!(
        "sqlite://{}",
        db_path.to_string_lossy().replace('\\', "/")
    );

    let pool = create_pool(&db_url)
        .await
        .map_err(|e| format!("database init failed: {e}"))?;

    match init_database(&pool).await {
        Ok(_) => Ok(pool),
        Err(sqlx::Error::Migrate(ref migrate_err)) if is_inconsistent(migrate_err) => {
            log::warn!(
                "migration state inconsistent: {migrate_err} — backing up and recreating database"
            );
            pool.close().await;

            // Backup existing DB so user data isn't silently discarded
            let bak = db_path.with_extension("db.bak");
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

fn is_inconsistent(err: &MigrateError) -> bool {
    matches!(
        err,
        MigrateError::VersionMissing(_) | MigrateError::VersionMismatch(_)
    )
}
