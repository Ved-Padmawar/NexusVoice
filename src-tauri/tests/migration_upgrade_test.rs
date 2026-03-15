/// Migration upgrade tests
///
/// 1. Normal upgrade: 0001+0002 → add 0003, data survives
/// 2. Recovery: simulate inconsistent migration state, verify backup+recreate path works
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

/// Run a single SQL file against the pool (no sqlx migrate machinery — raw exec).
async fn apply_sql(pool: &SqlitePool, sql: &str) {
    for statement in sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
            sqlx::query(stmt).execute(pool).await.unwrap();
        }
    }
}

#[tokio::test]
async fn test_migration_0003_upgrade_preserves_data() {
    // ── 1. Open an in-memory DB ──────────────────────────────────────────────
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await.unwrap();

    // ── 2. Apply 0001 + 0002 only (pre-upgrade state) ───────────────────────
    let sql_0001 = include_str!("../src/database/migrations/0001_init.sql");
    let sql_0002 = include_str!("../src/database/migrations/0002_refresh_tokens.sql");
    apply_sql(&pool, sql_0001).await;
    apply_sql(&pool, sql_0002).await;

    // ── 3. Insert seed data ──────────────────────────────────────────────────
    sqlx::query("INSERT INTO users (email, password_hash, created_at) VALUES ('test@example.com', 'hash123', '2024-01-01')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO transcripts (content, created_at) VALUES ('hello world', '2024-01-01')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO dictionary (term, replacement, created_at) VALUES ('teh', 'the', '2024-01-01')")
        .execute(&pool).await.unwrap();

    // ── 4. Apply 0003 (upgrade) ──────────────────────────────────────────────
    let sql_0003 = include_str!("../src/database/migrations/0003_word_frequency.sql");
    apply_sql(&pool, sql_0003).await;

    // ── 5. Assert word_frequency table exists and is empty ───────────────────
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM word_frequency")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "word_frequency should be empty after migration");

    // ── 6. Assert all existing data survived ────────────────────────────────
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(user_count, 1, "user data should survive upgrade");

    let transcript_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transcripts")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(transcript_count, 1, "transcript data should survive upgrade");

    let dict_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dictionary")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(dict_count, 1, "dictionary data should survive upgrade");

    // ── 7. Assert word_frequency is functional ───────────────────────────────
    sqlx::query("INSERT INTO word_frequency (word, count, dismissed) VALUES ('nexus', 1, 0)")
        .execute(&pool).await.unwrap();
    let word: String = sqlx::query_scalar("SELECT word FROM word_frequency WHERE word = 'nexus'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(word, "nexus");
}

#[tokio::test]
async fn test_migration_recovery_backup_and_recreate() {
    // Verify the recovery strategy: if migration state is inconsistent,
    // backup the DB file and recreate from scratch with all migrations.
    // This test simulates the recovery manually (no lib export needed).

    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("nexusvoice.db");
    let bak_path = dir.path().join("nexusvoice.db.bak");

    // ── 1. Create a "corrupted" DB: tables exist but _sqlx_migrations is wrong ──
    let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let opts = SqliteConnectOptions::from_str(&db_url)
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await.unwrap();

    let sql_0001 = include_str!("../src/database/migrations/0001_init.sql");
    apply_sql(&pool, sql_0001).await;

    // Insert data we care about
    sqlx::query("INSERT INTO users (email, password_hash, created_at) VALUES ('keep@me.com', 'h', '2024-01-01')")
        .execute(&pool).await.unwrap();

    pool.close().await;

    // ── 2. Backup + delete + recreate (mirrors open_database recovery path) ──
    std::fs::copy(&db_path, &bak_path).unwrap();
    std::fs::remove_file(&db_path).unwrap();

    assert!(bak_path.exists(), "backup should exist");
    assert!(!db_path.exists(), "original should be deleted");

    // ── 3. Fresh DB with all migrations ─────────────────────────────────────
    let fresh_opts = SqliteConnectOptions::from_str(&db_url)
        .unwrap()
        .create_if_missing(true);
    let fresh_pool = SqlitePool::connect_with(fresh_opts).await.unwrap();
    let sql_0002 = include_str!("../src/database/migrations/0002_refresh_tokens.sql");
    let sql_0003 = include_str!("../src/database/migrations/0003_word_frequency.sql");
    apply_sql(&fresh_pool, sql_0001).await;
    apply_sql(&fresh_pool, sql_0002).await;
    apply_sql(&fresh_pool, sql_0003).await;

    // Fresh DB is clean — all tables exist, no stale data
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&fresh_pool).await.unwrap();
    assert_eq!(user_count, 0, "fresh DB has no user rows");

    let wf_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM word_frequency")
        .fetch_one(&fresh_pool).await.unwrap();
    assert_eq!(wf_count, 0, "word_frequency table exists and is empty");

    // Backup contains the original data
    let bak_url = format!("sqlite://{}", bak_path.to_string_lossy().replace('\\', "/"));
    let bak_opts = SqliteConnectOptions::from_str(&bak_url).unwrap();
    let bak_pool = SqlitePool::connect_with(bak_opts).await.unwrap();
    let bak_user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&bak_pool).await.unwrap();
    assert_eq!(bak_user_count, 1, "backup preserves original user data");
}
