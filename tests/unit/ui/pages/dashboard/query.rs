use chrono::NaiveDate;

use super::*;

fn row(id: i64) -> TranscriptResponse {
    TranscriptResponse {
        id,
        content: format!("row {id}"),
        word_count: 2,
        duration_seconds: None,
        target_app: None,
        created_at: format!("2026-01-01 00:00:{id:02}"),
    }
}

fn rows(range: std::ops::RangeInclusive<i64>) -> Vec<TranscriptResponse> {
    range.map(row).collect()
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, d).unwrap()
}

#[test]
fn default_filters_ask_for_the_newest_first_page() {
    let query = TranscriptFilters::default().page_query(None);
    assert_eq!(query.limit, Some(50));
    assert_eq!(query.sort_asc, Some(false));
    assert!(query.from.is_none() && query.to.is_none());
    assert!(query.cursor_created_at.is_none() && query.cursor_id.is_none());
}

#[test]
fn filters_map_to_bare_dates_and_sort_order() {
    let filters = TranscriptFilters {
        from: Some(day(2)),
        to: Some(day(9)),
        sort_asc: true,
    };
    let query = filters.page_query(None);
    assert_eq!(query.from.as_deref(), Some("2026-03-02"));
    assert_eq!(query.to.as_deref(), Some("2026-03-09"));
    assert_eq!(query.sort_asc, Some(true));
}

#[test]
fn the_cursor_carries_both_halves() {
    let cursor = Cursor {
        created_at: "2026-01-01 00:00:50".into(),
        id: 50,
    };
    let query = TranscriptFilters::default().page_query(Some(&cursor));
    assert_eq!(
        query.cursor_created_at.as_deref(),
        Some("2026-01-01 00:00:50")
    );
    assert_eq!(query.cursor_id, Some(50));
}

#[test]
fn only_a_non_default_filter_counts_as_active() {
    assert!(!TranscriptFilters::default().is_active());
    assert!(TranscriptFilters {
        sort_asc: true,
        ..Default::default()
    }
    .is_active());
    assert!(TranscriptFilters {
        from: Some(day(1)),
        ..Default::default()
    }
    .is_active());
    assert!(TranscriptFilters {
        to: Some(day(1)),
        ..Default::default()
    }
    .is_active());
}

#[test]
fn a_full_page_points_at_its_last_row_and_a_short_one_ends_the_feed() {
    let full = Loaded::from_first_page(rows(1..=50));
    assert_eq!(
        full.next_cursor(),
        Some(&Cursor {
            created_at: "2026-01-01 00:00:50".into(),
            id: 50
        })
    );
    assert!(full.has_more());

    let short = Loaded::from_first_page(rows(1..=49));
    assert!(!short.has_more());
    assert!(!Loaded::from_first_page(Vec::new()).has_more());
}

#[test]
fn a_page_that_repeats_a_row_shows_it_once() {
    let mut loaded = Loaded::from_first_page(rows(1..=50));
    loaded.append_page(vec![row(50)]);
    let count = loaded.items.iter().filter(|t| t.id == 50).count();
    assert_eq!(count, 1);
    assert_eq!(loaded.items.len(), 50);
    assert!(!loaded.has_more(), "a one-row page is the last");
}

#[test]
fn appending_moves_the_cursor_to_the_new_last_row() {
    let mut loaded = Loaded::from_first_page(rows(1..=50));
    loaded.append_page(rows(51..=100));
    assert_eq!(loaded.next_cursor().map(|c| c.id), Some(100));
    assert_eq!(loaded.items.len(), 100);
}

#[test]
fn deleting_a_row_keeps_the_cursor_so_the_next_page_is_still_fetched() {
    let mut loaded = Loaded::from_first_page(rows(1..=50));
    assert_eq!(loaded.remove(50), Some(49));
    assert_eq!(loaded.items.len(), 49);
    assert_eq!(loaded.next_cursor().map(|c| c.id), Some(50));
    assert_eq!(loaded.remove(999), None);
}

#[test]
fn a_new_transcript_goes_on_top_once_and_never_moves_the_cursor() {
    let mut loaded = Loaded::from_first_page(rows(2..=51));
    assert!(loaded.prepend(row(60)));
    assert_eq!(loaded.items[0].id, 60);
    assert_eq!(loaded.items.len(), 51);
    assert_eq!(loaded.next_cursor().map(|c| c.id), Some(51));

    assert!(!loaded.prepend(row(60)), "the same event twice is ignored");
    assert_eq!(loaded.items.len(), 51);
}
