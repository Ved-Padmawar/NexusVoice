use super::{keyset_sql, Cursor, Filter, TranscriptRepository};
use crate::data::connection::init_database;
use crate::data::dto::transcript::CreateTranscript;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

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

/// Entries 1 to 6, one a day from 2026-01-01, alternating Slack and Notes.
async fn entries_in_two_apps(pool: &SqlitePool) {
    for i in 1..=6 {
        let id = insert(
            pool,
            &format!("notes {i}"),
            &format!("2026-01-0{i} 00:00:00"),
        )
        .await;
        sqlx::query("UPDATE transcripts SET target_app = ? WHERE id = ?")
            .bind(if i % 2 == 1 { "Slack" } else { "Notes" })
            .bind(id)
            .execute(pool)
            .await
            .expect("app");
    }
}

#[tokio::test]
async fn daily_words_sums_each_day_in_the_window_oldest_first() {
    let pool = pool().await;
    sqlx::query(
        "INSERT INTO transcripts (content, word_count, created_at) VALUES
         ('t', 5, '2026-03-10 10:00:00'), ('t', 7, '2026-03-10 10:01:00'),
         ('t', 3, '2026-03-08 10:00:00'), ('t', 100, '2026-01-25 10:00:00'),
         ('t', 100, '2026-03-11 00:00:00')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let rows = TranscriptRepository::new(pool)
        .daily_words(30, 0, "2026-03-10")
        .await
        .expect("query");
    let words: Vec<i64> = rows.iter().map(|(_, w)| *w).collect();
    assert_eq!(words, [3, 12]);
    assert!(rows[0].0 < rows[1].0, "oldest first: {rows:?}");
}

#[tokio::test]
async fn daily_words_counts_by_the_callers_local_day() {
    let pool = pool().await;
    insert(&pool, "today", "2026-03-09 10:00:00").await;
    insert(&pool, "yesterday", "2026-03-09 09:59:59").await;
    let ahead = TranscriptRepository::new(pool)
        .daily_words(1, 14 * 60, "2026-03-10")
        .await
        .expect("query");
    assert_eq!(ahead, [(String::from("2026-03-10"), 1)]);
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
            .list_keyset(3, c, &Filter::default(), true)
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
async fn keyset_pages_seek_the_index_instead_of_scanning() {
    let pool = pool().await;
    let cursor = Cursor {
        created_at: "2026-01-01 00:00:00",
        id: 1,
    };
    let dates = Filter {
        from: Some("2026-01-01 00:00:00"),
        to: Some("2026-01-31 23:59:59"),
        app: None,
    };
    let app = Filter {
        app: Some("Slack"),
        ..dates
    };
    // A cursor page, a first page under a date filter, and both under an app.
    for (sort_desc, cursor, filter) in [
        (true, Some(cursor), Filter::default()),
        (false, Some(cursor), Filter::default()),
        (true, None, dates),
        (false, None, dates),
        (true, Some(cursor), app),
        (false, None, app),
    ] {
        let sql = format!(
            "EXPLAIN QUERY PLAN {}",
            keyset_sql(sort_desc, cursor, &filter)
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        for value in [filter.app, filter.from, filter.to].into_iter().flatten() {
            q = q.bind(value);
        }
        if let Some(c) = cursor {
            q = q.bind(c.created_at).bind(c.id);
        }
        let rows = q.bind(50_i64).fetch_all(&pool).await.expect("plan");
        let plan: Vec<String> = rows.iter().map(|r| r.get("detail")).collect();
        assert!(
            plan.iter().any(|d| d.starts_with("SEARCH")),
            "page must seek the keyset index, got {plan:?}"
        );
    }
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
        .list_keyset(2, None, &Filter::default(), true)
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
            &Filter::default(),
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
        .list_keyset(3, None, &Filter::default(), true)
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
            &Filter::default(),
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
        .list_keyset(2, None, &Filter::default(), false)
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
            &Filter::default(),
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
            &Filter {
                from: Some("2026-01-02 00:00:00"),
                to: Some("2026-01-05 00:00:00"),
                app: None,
            },
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
        .search("\"meeting\"", 3, None, &Filter::default(), true)
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
            &Filter::default(),
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
        .search("\"report\"", 2, None, &Filter::default(), false)
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
            &Filter {
                from: Some("2026-01-02 00:00:00"),
                to: Some("2026-01-04 23:59:59"),
                app: None,
            },
            true,
        )
        .await
        .expect("page");
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![4, 3, 2]);
}

#[tokio::test]
async fn keyset_keeps_to_one_app_across_dates_and_cursor() {
    let pool = pool().await;
    entries_in_two_apps(&pool).await;
    let repo = TranscriptRepository::new(pool);

    let rows = repo
        .list_keyset(
            10,
            Some(Cursor {
                created_at: "2026-01-05 00:00:00",
                id: 5,
            }),
            &Filter {
                from: Some("2026-01-02 00:00:00"),
                to: None,
                app: Some("Slack"),
            },
            true,
        )
        .await
        .expect("page");
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![3]);
}

#[tokio::test]
async fn search_keeps_to_one_app_within_the_dates() {
    let pool = pool().await;
    entries_in_two_apps(&pool).await;
    let repo = TranscriptRepository::new(pool);

    let rows = repo
        .search(
            "\"notes\"",
            10,
            None,
            &Filter {
                from: Some("2026-01-02 00:00:00"),
                to: Some("2026-01-05 23:59:59"),
                app: Some("Notes"),
            },
            true,
        )
        .await
        .expect("page");
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![4, 2]);
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
        .list_keyset(
            10,
            None,
            &Filter {
                from: Some("2026-01-15"),
                to: Some("2026-01-15"),
                app: None,
            },
            true,
        )
        .await
        .expect("narrow");
    assert!(narrow.is_empty(), "bare bound drops the whole day");

    let widened = repo
        .list_keyset(
            10,
            None,
            &Filter {
                from: Some("2026-01-15"),
                to: Some("2026-01-15 23:59:59"),
                app: None,
            },
            true,
        )
        .await
        .expect("widened");
    assert_eq!(widened.len(), 2);
}

// ── build_fts_query ────────────────────────────────────────────────────
// User input becomes FTS5 MATCH syntax. Unquoted, a term like "AND" or an
// embedded quote is read as syntax and errors the whole query.

#[test]
fn fts_query_quotes_each_term_and_lowercases_it() {
    // The star sits outside the quotes: `"term*"` matches a literal asterisk,
    // `"term"*` is the prefix operator.
    assert_eq!(
        TranscriptRepository::build_fts_query("Meeting"),
        "\"meeting\" OR \"meeting\"*"
    );
}

#[test]
fn fts_query_omits_the_prefix_variant_for_very_short_terms() {
    // A 1-2 char prefix matches most of the table; exact match only.
    assert_eq!(TranscriptRepository::build_fts_query("ok"), "\"ok\"");
}

#[test]
fn fts_query_escapes_embedded_quotes_by_doubling_them() {
    // An unescaped quote would terminate the literal and make the rest syntax.
    let q = TranscriptRepository::build_fts_query("say \"hi\"");
    assert!(q.contains("\"\"hi\"\""), "{q}");
}

#[test]
fn fts_query_drops_tokens_with_no_alphanumerics() {
    // A lone "-" would produce an empty quoted literal, which FTS5 rejects.
    assert_eq!(TranscriptRepository::build_fts_query("- ?? !"), "");
    assert_eq!(
        TranscriptRepository::build_fts_query("notes -"),
        "\"notes\" OR \"notes\"*"
    );
}

#[test]
fn fts_query_joins_multiple_terms_with_or() {
    let q = TranscriptRepository::build_fts_query("alpha beta");
    assert_eq!(q, "\"alpha\" OR \"alpha\"* OR \"beta\" OR \"beta\"*");
}

/// The end-to-end proof: whatever `build_fts_query` emits must be syntax SQLite
/// accepts. A quoting regression shows up here as a DB error, not a wrong count.
#[tokio::test]
async fn raw_user_input_produces_a_query_sqlite_accepts() {
    let pool = pool().await;
    insert(&pool, "the meeting notes", "2026-01-01 00:00:00").await;
    insert(&pool, "say \"hi\" to everyone", "2026-01-02 00:00:00").await;
    let repo = TranscriptRepository::new(pool);

    for raw in [
        "meeting",
        "meet",       // prefix match
        "say \"hi\"", // embedded quotes
        "AND",        // an FTS5 operator as a search word
        "NOT OR AND",
        "notes*",
        "a -- b",
        "C++",
    ] {
        let fts = TranscriptRepository::build_fts_query(raw);
        if fts.trim().is_empty() {
            continue;
        }
        repo.search(&fts, 10, None, &Filter::default(), true)
            .await
            .unwrap_or_else(|e| panic!("user input {raw:?} produced invalid FTS {fts:?}: {e}"));
    }
}

#[tokio::test]
async fn a_prefix_search_finds_a_partially_typed_word() {
    let pool = pool().await;
    insert(&pool, "quarterly projections", "2026-01-01 00:00:00").await;
    let repo = TranscriptRepository::new(pool);

    let fts = TranscriptRepository::build_fts_query("project");
    let rows = repo
        .search(&fts, 10, None, &Filter::default(), true)
        .await
        .expect("search");
    assert_eq!(rows.len(), 1, "prefix match should find 'projections'");
}

#[tokio::test]
async fn releasing_audio_skips_pinned_entries_and_never_the_newest() {
    let pool = pool().await;
    // Oldest first: a pinned one, two unpinned with audio, one unpinned without, the newest with audio.
    let pinned = insert(&pool, "pinned", "2024-01-01 10:00:00").await;
    let old = insert(&pool, "old", "2024-01-02 10:00:00").await;
    let mid = insert(&pool, "mid", "2024-01-03 10:00:00").await;
    insert(&pool, "text only", "2024-01-04 10:00:00").await;
    let newest = insert(&pool, "newest", "2024-01-05 10:00:00").await;
    let repo = TranscriptRepository::new(pool);
    for id in [pinned, old, mid, newest] {
        repo.set_has_audio(id, true).await.unwrap();
    }
    repo.set_pinned(pinned, true).await.unwrap();

    // Two kept: the pinned one doesn't count, the entry without audio can't.
    assert_eq!(repo.release_audio_beyond(Some(2)).await.unwrap(), [old]);
    assert_eq!(repo.release_audio_beyond(Some(1)).await.unwrap(), [mid]);
    assert_eq!(repo.release_audio_beyond(None).await.unwrap(), [newest]);
    let has_audio = |id| {
        let repo = &repo;
        async move { repo.get_by_id(id).await.unwrap().unwrap().has_audio }
    };
    assert!(has_audio(pinned).await);
    assert!(!has_audio(old).await);
}

#[tokio::test]
async fn an_undo_applies_only_to_the_rewrite_it_belongs_to() {
    let pool = pool().await;
    let id = insert(&pool, "original", "2024-01-01 10:00:00").await;
    let repo = TranscriptRepository::new(pool);
    let (original, original_model, first) = repo
        .set_text(id, "same words", "model-a")
        .await
        .unwrap()
        .unwrap();
    // A newer retry with another model that happens to produce the same words.
    let (replaced, replaced_model, second) = repo
        .set_text(id, "same words", "model-b")
        .await
        .unwrap()
        .unwrap();

    assert!(!repo
        .restore_text(id, first.revision, &original, original_model.as_deref())
        .await
        .unwrap());
    assert!(repo
        .restore_text(id, second.revision, &replaced, replaced_model.as_deref())
        .await
        .unwrap());
    let entry = repo.get_by_id(id).await.unwrap().unwrap();
    assert_eq!(entry.model.as_deref(), Some("model-a"));
    let (replaced, replaced_model, _) = repo
        .set_text(id, "new words", "model-c")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (replaced.as_str(), replaced_model.as_deref()),
        ("same words", Some("model-a"))
    );
}

fn created(content: &str) -> CreateTranscript {
    CreateTranscript {
        content: content.into(),
        word_count: 2,
        duration_seconds: None,
        target_app: None,
        source_name: None,
        model: None,
        pending: false,
        language: None,
    }
}

#[tokio::test]
async fn a_write_takes_the_history_revisions_it_changes_and_reads_take_none() {
    // The windows only reread History when told: a silent write leaves them
    // stale, and a needless one rereads every page and the totals.
    let repo = TranscriptRepository::new(pool().await);
    let revs = || {
        (
            repo.history.rows.committed(),
            repo.history.totals.committed(),
        )
    };
    let entry = repo.create(created("one two")).await.expect("create");

    let before = revs();
    repo.set_pinned(entry.id, true).await.expect("pin");
    let pinned = revs();
    assert!(pinned.0.rev > before.0.rev);
    assert_eq!(pinned.1, before.1);

    repo.set_pinned(entry.id, true).await.expect("pin again");
    repo.get_by_id(entry.id).await.expect("read");
    repo.get_stats().await.expect("stats");
    repo.delete_by_id(entry.id + 1)
        .await
        .expect("delete nothing");
    assert_eq!(revs(), pinned);

    repo.set_text(entry.id, "three", "whisper-small")
        .await
        .expect("set text");
    let rewritten = revs();
    assert!(rewritten.0.rev > pinned.0.rev);
    assert_eq!(rewritten.0, rewritten.1);
}

#[tokio::test]
async fn search_finds_a_rewritten_entry_by_its_new_text_only() {
    let repo = TranscriptRepository::new(pool().await);
    let entry = repo.create(created("walrus tusk")).await.expect("create");
    repo.set_pinned(entry.id, true).await.expect("pin");
    repo.set_text(entry.id, "narwhal horn", "whisper-small")
        .await
        .expect("set text");

    let find = |word: &str| {
        let query = TranscriptRepository::build_fts_query(word);
        let repo = repo.clone();
        async move {
            repo.search(&query, 10, None, &Filter::default(), true)
                .await
                .expect("search")
                .len()
        }
    };
    assert_eq!(find("narwhal").await, 1);
    assert_eq!(find("walrus").await, 0);
}
