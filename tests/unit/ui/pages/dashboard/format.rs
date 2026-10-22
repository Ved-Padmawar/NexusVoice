use chrono::{FixedOffset, NaiveDate};

use super::*;

fn transcript(id: i64, content: &str, created_at: &str, duration: Option<f64>) -> TranscriptResponse {
    TranscriptResponse {
        id,
        content: content.to_string(),
        word_count: 3,
        duration_seconds: duration,
        target_app: None,
        created_at: created_at.to_string(),
    }
}

#[test]
fn fmt_time_picks_the_two_largest_units_and_drops_zero_parts() {
    assert_eq!(fmt_time(0), "0s");
    assert_eq!(fmt_time(59), "59s");
    assert_eq!(fmt_time(60), "1m");
    assert_eq!(fmt_time(185), "3m 5s");
    assert_eq!(fmt_time(3600), "1h");
    assert_eq!(fmt_time(3600 + 59), "1h"); // seconds are dropped past an hour
    assert_eq!(fmt_time(5400), "1h 30m");
}

#[test]
fn fmt_count_groups_thousands() {
    assert_eq!(fmt_count(0), "0");
    assert_eq!(fmt_count(999), "999");
    assert_eq!(fmt_count(1000), "1,000");
    assert_eq!(fmt_count(1_234_567), "1,234,567");
    assert_eq!(fmt_count(-12_345), "-12,345");
}

#[test]
fn fmt_date_reads_sqlite_timestamps_as_utc() {
    let tz = FixedOffset::east_opt(2 * 3600).unwrap();
    assert_eq!(fmt_date_in("2026-06-05 12:30:00", &tz), "Jun 5, 02:30 PM");
    // A zone that crosses midnight moves the day too.
    let west = FixedOffset::west_opt(5 * 3600).unwrap();
    assert_eq!(fmt_date_in("2026-01-01 02:05:09", &west), "Dec 31, 09:05 PM");
}

#[test]
fn fmt_date_accepts_fractional_seconds_and_rfc3339() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(fmt_date_in("2026-06-05 00:00:00.250", &utc), "Jun 5, 12:00 AM");
    assert_eq!(fmt_date_in("2026-06-05T13:00:00+02:00", &utc), "Jun 5, 11:00 AM");
}

#[test]
fn fmt_date_flags_garbage_instead_of_guessing() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(fmt_date_in("yesterday", &utc), "Invalid Date");
}

#[test]
fn count_noun_pluralises_except_one() {
    assert_eq!(count_noun(0, "transcript"), "0 transcripts");
    assert_eq!(count_noun(1, "transcript"), "1 transcript");
    assert_eq!(count_noun(2, "transcript"), "2 transcripts");
}

#[test]
fn export_file_name_carries_the_date_and_extension() {
    let day = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
    assert_eq!(ExportFormat::Txt.file_name(day), "nexusvoice-transcripts-2026-03-09.txt");
    assert_eq!(ExportFormat::Json.file_name(day), "nexusvoice-transcripts-2026-03-09.json");
}

#[test]
fn txt_export_separates_entries_with_a_rule() {
    let utc = FixedOffset::east_opt(0).unwrap();
    let items = [
        transcript(2, "second one", "2026-06-05 12:30:00", None),
        transcript(1, "line one\nline two", "2026-06-04 08:00:00", None),
    ];
    let text = export_content(ExportFormat::Txt, &items, &utc).unwrap();
    assert_eq!(
        text,
        "[Jun 5, 12:30 PM]\nsecond one\n\n---\n\n[Jun 4, 08:00 AM]\nline one\nline two"
    );
}

#[test]
fn json_export_keeps_the_legacy_shape_and_key_order() {
    let utc = FixedOffset::east_opt(0).unwrap();
    let items = [
        transcript(7, "hi \"there\"", "2026-06-05 12:30:00", Some(2.0)),
        transcript(8, "yo", "2026-06-05 12:31:00", Some(1.5)),
        transcript(9, "no timing", "2026-06-05 12:32:00", None),
    ];
    let json = export_content(ExportFormat::Json, &items, &utc).unwrap();
    let expected = r#"[
  {
    "id": 7,
    "content": "hi \"there\"",
    "createdAt": "2026-06-05 12:30:00",
    "wordCount": 3,
    "durationSeconds": 2
  },
  {
    "id": 8,
    "content": "yo",
    "createdAt": "2026-06-05 12:31:00",
    "wordCount": 3,
    "durationSeconds": 1.5
  },
  {
    "id": 9,
    "content": "no timing",
    "createdAt": "2026-06-05 12:32:00",
    "wordCount": 3,
    "durationSeconds": null
  }
]"#;
    assert_eq!(json, expected);
}

#[test]
fn exporting_nothing_yields_an_empty_document() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(export_content(ExportFormat::Txt, &[], &utc).unwrap(), "");
    assert_eq!(export_content(ExportFormat::Json, &[], &utc).unwrap(), "[]");
}
