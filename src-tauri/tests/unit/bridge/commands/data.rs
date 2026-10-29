//! `PageQuery` — the pagination/filter contract between the frontend and the
//! keyset queries. Pure argument shaping, so it is tested without a DB.

use super::*;

fn query(limit: Option<i64>) -> PageQuery {
    PageQuery {
        limit,
        ..PageQuery::default()
    }
}

#[test]
fn limit_defaults_when_the_client_sends_none() {
    assert_eq!(query(None).limit(), DEFAULT_PAGE_SIZE);
}

#[test]
fn limit_is_clamped_to_the_page_ceiling() {
    // Without the clamp a client could pull the whole table into memory.
    assert_eq!(query(Some(10_000)).limit(), MAX_PAGE_SIZE);
    assert_eq!(query(Some(MAX_PAGE_SIZE)).limit(), MAX_PAGE_SIZE);
}

#[test]
fn limit_is_clamped_to_at_least_one() {
    // LIMIT 0 returns nothing and LIMIT -1 is unbounded in SQLite — both would
    // read as "the list ended" and stop the infinite scroll.
    assert_eq!(query(Some(0)).limit(), 1);
    assert_eq!(query(Some(-5)).limit(), 1);
}

#[test]
fn a_cursor_needs_both_halves() {
    // Either half alone is a client bug; a half-cursor must not silently
    // page from an arbitrary position.
    let mut q = PageQuery::default();
    assert!(q.cursor().is_none());

    q.cursor_created_at = Some("2026-01-01 00:00:00".to_string());
    assert!(q.cursor().is_none(), "created_at alone is not a cursor");

    q.cursor_created_at = None;
    q.cursor_id = Some(7);
    assert!(q.cursor().is_none(), "id alone is not a cursor");

    q.cursor_created_at = Some("2026-01-01 00:00:00".to_string());
    let cursor = q.cursor().expect("both halves present");
    assert_eq!(cursor.created_at, "2026-01-01 00:00:00");
    assert_eq!(cursor.id, 7);
}

#[test]
fn a_bare_end_date_is_widened_to_end_of_day() {
    // `created_at` is "YYYY-MM-DD HH:MM:SS" compared lexicographically, so the
    // picker's bare "YYYY-MM-DD" would exclude the day it names. The repository
    // tests assume this widening already happened — this is what does it.
    let q = PageQuery {
        to: Some("2026-01-15".to_string()),
        ..PageQuery::default()
    };
    assert_eq!(q.end_bound().as_deref(), Some("2026-01-15 23:59:59"));
}

#[test]
fn an_explicit_end_timestamp_is_passed_through() {
    let mut q = PageQuery {
        to: Some("2026-01-15 09:30:00".to_string()),
        ..PageQuery::default()
    };
    assert_eq!(q.end_bound().as_deref(), Some("2026-01-15 09:30:00"));

    q.to = None;
    assert_eq!(q.end_bound(), None);
}

#[test]
fn the_start_bound_is_never_rewritten() {
    // A bare start date already sorts before every time on that day.
    let q = PageQuery {
        from: Some("2026-01-15".to_string()),
        ..PageQuery::default()
    };
    assert_eq!(q.start_bound(), Some("2026-01-15"));
}

#[test]
fn sort_defaults_to_newest_first() {
    assert!(
        PageQuery::default().sort_desc(),
        "history reads newest-first"
    );

    let mut q = PageQuery {
        sort_asc: Some(true),
        ..PageQuery::default()
    };
    assert!(!q.sort_desc());
    q.sort_asc = Some(false);
    assert!(q.sort_desc());
}
