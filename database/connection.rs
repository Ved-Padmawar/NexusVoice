use std::time::Duration;

use sqlx::{
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
  sqlx::migrate!("../database/migrations")
    .run(pool)
    .await
    .map_err(|err| sqlx::Error::Migrate(Box::new(err)))
}
