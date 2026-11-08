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

fn days(from: Option<&str>, to: Option<&str>) -> PageQuery {
    PageQuery {
        from: from.map(str::to_string),
        to: to.map(str::to_string),
        ..PageQuery::default()
    }
}

fn zone(hours: i32, minutes: i32) -> chrono::FixedOffset {
    chrono::FixedOffset::east_opt((hours * 60 + minutes) * 60).unwrap()
}

#[test]
fn a_day_east_of_utc_starts_the_evening_before() {
    // `created_at` is UTC: in India, 00:10 on the 15th is stored as the 14th.
    let range = days(Some("2026-01-15"), Some("2026-01-15"))
        .created_at_range_in(&zone(5, 30))
        .unwrap();
    assert_eq!(range.start.as_deref(), Some("2026-01-14 18:30:00"));
    assert_eq!(range.end.as_deref(), Some("2026-01-15 18:29:59"));
}

#[test]
fn a_day_west_of_utc_runs_into_the_next_utc_date() {
    // In California, 23:50 on the 15th is stored as the 16th.
    let range = days(Some("2026-01-15"), Some("2026-01-15"))
        .created_at_range_in(&zone(-8, 0))
        .unwrap();
    assert_eq!(range.start.as_deref(), Some("2026-01-15 08:00:00"));
    assert_eq!(range.end.as_deref(), Some("2026-01-16 07:59:59"));
}

#[test]
fn a_range_spans_from_its_first_day_to_the_end_of_its_last() {
    let range = days(Some("2026-01-10"), Some("2026-01-15"))
        .created_at_range_in(&zone(2, 0))
        .unwrap();
    assert_eq!(range.start.as_deref(), Some("2026-01-09 22:00:00"));
    assert_eq!(range.end.as_deref(), Some("2026-01-15 21:59:59"));
}

#[test]
fn absent_days_leave_that_side_open() {
    let range = days(None, None).created_at_range_in(&zone(0, 0)).unwrap();
    assert!(range.start.is_none() && range.end.is_none());

    let range = days(Some("2026-01-15"), None)
        .created_at_range_in(&zone(0, 0))
        .unwrap();
    assert_eq!(range.start.as_deref(), Some("2026-01-15 00:00:00"));
    assert!(range.end.is_none());
}

#[test]
fn anything_but_a_calendar_day_is_rejected() {
    for bad in ["2026-01-15 09:30:00", "15/01/2026", "2026-02-30", ""] {
        assert!(
            days(Some(bad), None)
                .created_at_range_in(&zone(0, 0))
                .is_err(),
            "{bad:?} must be rejected"
        );
        assert!(
            days(None, Some(bad))
                .created_at_range_in(&zone(0, 0))
                .is_err(),
            "{bad:?} must be rejected"
        );
    }
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
