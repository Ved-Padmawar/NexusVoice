use serial_test::serial;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

/// Via the migrator, not a hand `;` split — the triggers contain semicolons.
async fn apply_schema(pool: &SqlitePool) {
    sqlx::migrate!("src/database/migrations")
        .run(pool)
        .await
        .expect("schema applies");
}

async fn table_exists(pool: &SqlitePool, name: &str) -> bool {
    let found: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name = ?")
            .bind(name)
            .fetch_optional(pool)
            .await
            .unwrap();
    found.is_some()
}

#[tokio::test]
#[serial]
async fn schema_has_the_expected_shape() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    apply_schema(&pool).await;

    assert!(table_exists(&pool, "transcripts").await);
    assert!(table_exists(&pool, "dictionary").await);
    assert!(table_exists(&pool, "transcripts_fts").await);

    assert!(!table_exists(&pool, "users").await, "users must be gone");
    assert!(
        !table_exists(&pool, "app_session").await,
        "app_session must be gone"
    );

    sqlx::query(
        "INSERT INTO transcripts (content, word_count, duration_seconds, target_app)
         VALUES ('hello world', 2, 1.5, 'VS Code')",
    )
    .execute(&pool)
    .await
    .expect("insert with every column");

    let hit: (i64,) =
        sqlx::query_as("SELECT count(*) FROM transcripts_fts WHERE transcripts_fts MATCH 'hello'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hit.0, 1, "FTS trigger should have indexed the insert");
}

#[tokio::test]
#[serial]
async fn recovery_backs_up_and_recreates() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("nexusvoice.db");
    let bak_path = dir.path().join("nexusvoice.db.bak");

    let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let opts = SqliteConnectOptions::from_str(&db_url)
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await.unwrap();

    apply_schema(&pool).await;
    sqlx::query("INSERT INTO transcripts (content, word_count) VALUES ('keep me', 2)")
        .execute(&pool)
        .await
        .unwrap();

    pool.close().await;

    std::fs::copy(&db_path, &bak_path).unwrap();
    // Windows holds the file handle briefly after close(), racing the delete
    // (os error 32), so retry with backoff.
    let mut delete_result = std::fs::remove_file(&db_path);
    for _ in 0..20 {
        if delete_result.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        delete_result = std::fs::remove_file(&db_path);
    }
    delete_result.unwrap();

    assert!(bak_path.exists(), "backup should exist");
    assert!(!db_path.exists(), "original should be deleted");

    let fresh_opts = SqliteConnectOptions::from_str(&db_url)
        .unwrap()
        .create_if_missing(true);
    let fresh_pool = SqlitePool::connect_with(fresh_opts).await.unwrap();
    apply_schema(&fresh_pool).await;

    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM transcripts")
        .fetch_one(&fresh_pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "fresh DB has no rows");

    let bak_url = format!("sqlite://{}", bak_path.to_string_lossy().replace('\\', "/"));
    let bak_opts = SqliteConnectOptions::from_str(&bak_url).unwrap();
    let bak_pool = SqlitePool::connect_with(bak_opts).await.unwrap();
    let bak_count: (i64,) = sqlx::query_as("SELECT count(*) FROM transcripts")
        .fetch_one(&bak_pool)
        .await
        .unwrap();
    assert_eq!(bak_count.0, 1, "backup preserves the original row");
}
