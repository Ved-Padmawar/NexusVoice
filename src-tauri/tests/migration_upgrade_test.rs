use serial_test::serial;
use sqlx::SqlitePool;

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
