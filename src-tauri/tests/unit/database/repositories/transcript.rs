use super::{Cursor, TranscriptRepository};
use crate::database::connection::init_database;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    init_database(&pool).await.expect("migrations");
    pool
}

/// Insert with an explicit timestamp — `create()` uses `CURRENT_TIMESTAMP`, which
/// can't produce the same-second ties these tests are about.
async fn insert(pool: &SqlitePool, content: &str, created_at: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO transcripts (content, word_count, created_at) VALUES (?, 1, ?) RETURNING id",
    )
    .bind(content)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .expect("insert")
}

#[tokio::test]
async fn keyset_walks_every_row_without_repeats() {
    let pool = pool().await;
    for i in 1..=10 {
        insert(&pool, &format!("t{i}"), &format!("2026-01-01 00:00:{i:02}")).await;
    }
    let repo = TranscriptRepository::new(pool);

    let mut seen: Vec<i64> = Vec::new();
    let mut cursor: Option<(String, i64)> = None;
    loop {
        let c = cursor.as_ref().map(|(ts, id)| Cursor {
            created_at: ts,
            id: *id,
        });
        let rows = repo
            .list_keyset(3, c, None, None, true)
            .await
            .expect("page");
        if rows.is_empty() {
            break;
        }
        let last = rows.last().expect("non-empty");
        cursor = Some((last.created_at.to_string(), last.id));
        seen.extend(rows.iter().map(|r| r.id));
    }

    assert_eq!(seen.len(), 10, "every row returned exactly once: {seen:?}");
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 10, "no duplicates across pages: {seen:?}");
}

#[tokio::test]
async fn keyset_breaks_ties_on_id_within_one_second() {
    let pool = pool().await;
    // All four share a timestamp — created_at alone can't order them, so a
    // cursor without the id tiebreak would loop or skip.
    for i in 1..=4 {
        insert(&pool, &format!("t{i}"), "2026-01-01 00:00:00").await;
    }
    let repo = TranscriptRepository::new(pool);

    let first = repo
        .list_keyset(2, None, None, None, true)
        .await
        .expect("p1");
    assert_eq!(first.iter().map(|r| r.id).collect::<Vec<_>>(), vec![4, 3]);

    let last = first.last().expect("non-empty");
    let last_ts = last.created_at.to_string();
    let second = repo
        .list_keyset(
            2,
            Some(Cursor {
                created_at: &last_ts,
                id: last.id,
            }),
            None,
            None,
            true,
        )
        .await
        .expect("p2");
    assert_eq!(second.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 1]);
}

#[tokio::test]
async fn keyset_is_stable_when_a_row_is_inserted_mid_scroll() {
    let pool = pool().await;
    for i in 1..=6 {
        insert(&pool, &format!("t{i}"), &format!("2026-01-01 00:00:{i:02}")).await;
    }
    let repo = TranscriptRepository::new(pool.clone());

    let first = repo
        .list_keyset(3, None, None, None, true)
        .await
        .expect("p1");
    assert_eq!(
        first.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![6, 5, 4]
    );

    // A transcription finishes while the user is mid-scroll. With OFFSET 3 this
    // shifts every row down one and page 2 re-serves id 4.
    insert(&pool, "new", "2026-01-01 00:00:99").await;

    let last = first.last().expect("non-empty");
    let last_ts = last.created_at.to_string();
    let second = repo
        .list_keyset(
            3,
            Some(Cursor {
                created_at: &last_ts,
                id: last.id,
            }),
            None,
            None,
            true,
        )
        .await
        .expect("p2");
    assert_eq!(
        second.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "insert during scroll must not shift the window"
    );
}

#[tokio::test]
async fn keyset_ascending_walks_forward() {
    let pool = pool().await;
    for i in 1..=4 {
        insert(&pool, &format!("t{i}"), &format!("2026-01-01 00:00:{i:02}")).await;
    }
    let repo = TranscriptRepository::new(pool);

    let first = repo
        .list_keyset(2, None, None, None, false)
        .await
        .expect("p1");
    assert_eq!(first.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 2]);

    let last = first.last().expect("non-empty");
    let last_ts = last.created_at.to_string();
    let second = repo
        .list_keyset(
            2,
            Some(Cursor {
                created_at: &last_ts,
                id: last.id,
            }),
            None,
            None,
            false,
        )
        .await
        .expect("p2");
    assert_eq!(second.iter().map(|r| r.id).collect::<Vec<_>>(), vec![3, 4]);
}

#[tokio::test]
async fn keyset_respects_date_range_with_cursor() {
    let pool = pool().await;
    for i in 1..=6 {
        insert(&pool, &format!("t{i}"), &format!("2026-01-0{i} 00:00:00")).await;
    }
    let repo = TranscriptRepository::new(pool);

    let rows = repo
        .list_keyset(
            10,
            Some(Cursor {
                created_at: "2026-01-05 00:00:00",
                id: 5,
            }),
            Some("2026-01-02 00:00:00"),
            Some("2026-01-05 00:00:00"),
            true,
        )
        .await
        .expect("page");
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![4, 3, 2]);
}

#[tokio::test]
async fn search_pages_newest_first_without_repeats() {
    let pool = pool().await;
    for i in 1..=6 {
        insert(
            &pool,
            &format!("meeting notes {i}"),
            &format!("2026-01-0{i} 00:00:00"),
        )
        .await;
    }
    let repo = TranscriptRepository::new(pool);

    let page1 = repo
        .search("\"meeting\"", 3, None, None, None, true)
        .await
        .expect("page 1");
    assert_eq!(
        page1.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![6, 5, 4]
    );

    // Search keys its cursor on id alone, so created_at here is inert.
    let last = page1.last().expect("row");
    let page2 = repo
        .search(
            "\"meeting\"",
            3,
            Some(Cursor {
                created_at: "",
                id: last.id,
            }),
            None,
            None,
            true,
        )
        .await
        .expect("page 2");
    assert_eq!(
        page2.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
}

#[tokio::test]
async fn search_ascending_walks_forward() {
    let pool = pool().await;
    for i in 1..=4 {
        insert(
            &pool,
            &format!("report {i}"),
            &format!("2026-01-0{i} 00:00:00"),
        )
        .await;
    }
    let repo = TranscriptRepository::new(pool);

    let rows = repo
        .search("\"report\"", 2, None, None, None, false)
        .await
        .expect("page");
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 2]);
}

#[tokio::test]
async fn search_respects_the_date_range() {
    let pool = pool().await;
    for i in 1..=5 {
        insert(
            &pool,
            &format!("invoice {i}"),
            &format!("2026-01-0{i} 12:00:00"),
        )
        .await;
    }
    let repo = TranscriptRepository::new(pool);

    // End bound is already widened to end-of-day by the command layer.
    let rows = repo
        .search(
            "\"invoice\"",
            10,
            None,
            Some("2026-01-02 00:00:00"),
            Some("2026-01-04 23:59:59"),
            true,
        )
        .await
        .expect("page");
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![4, 3, 2]);
}

/// A bare `YYYY-MM-DD` upper bound compares lexicographically against
/// `YYYY-MM-DD HH:MM:SS`, so without widening it excludes the day it names.
#[tokio::test]
async fn a_bare_end_date_would_exclude_its_own_day() {
    let pool = pool().await;
    insert(&pool, "morning", "2026-01-15 09:00:00").await;
    insert(&pool, "evening", "2026-01-15 21:00:00").await;
    let repo = TranscriptRepository::new(pool);

    let narrow = repo
        .list_keyset(10, None, Some("2026-01-15"), Some("2026-01-15"), true)
        .await
        .expect("narrow");
    assert!(narrow.is_empty(), "bare bound drops the whole day");

    let widened = repo
        .list_keyset(
            10,
            None,
            Some("2026-01-15"),
            Some("2026-01-15 23:59:59"),
            true,
        )
        .await
        .expect("widened");
    assert_eq!(widened.len(), 2);
}
