//! The dashboard's display and export text, UI-free for testing.

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Serialize, Serializer};

use crate::api::dto::TranscriptResponse;

/// "45s", "3m 5s", "2h", "1h 30m".
pub fn fmt_time(seconds: i64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let (minutes, rem) = (seconds / 60, seconds % 60);
    if minutes < 60 {
        return if rem > 0 {
            format!("{minutes}m {rem}s")
        } else {
            format!("{minutes}m")
        };
    }
    let (hours, rm) = (minutes / 60, minutes % 60);
    if rm > 0 {
        format!("{hours}h {rm}m")
    } else {
        format!("{hours}h")
    }
}

/// `1234567` as "1,234,567", like `toLocaleString()` in an English locale.
pub fn fmt_count(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

const SQLITE_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";

/// "Jun 5, 02:30 PM" in the local zone. `SQLite`'s `CURRENT_TIMESTAMP` is UTC
/// without a marker; the raw string stays as the pagination cursor.
pub fn fmt_date(raw: &str) -> String {
    fmt_date_in(raw, &Local)
}

pub fn fmt_date_in<Tz: TimeZone>(raw: &str, tz: &Tz) -> String {
    let utc = NaiveDateTime::parse_from_str(raw, SQLITE_FORMAT)
        .map(|naive| naive.and_utc())
        .or_else(|_| DateTime::parse_from_rfc3339(raw).map(|d| d.with_timezone(&Utc)));
    match utc {
        Ok(utc) => utc
            .with_timezone(tz)
            .naive_local()
            .format("%b %-d, %I:%M %p")
            .to_string(),
        Err(_) => "Invalid Date".to_string(),
    }
}

pub fn count_noun(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Txt,
    Json,
}

impl ExportFormat {
    pub const ALL: [Self; 2] = [Self::Txt, Self::Json];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Json => "json",
        }
    }

    pub fn menu_label(self) -> &'static str {
        match self {
            Self::Txt => "Plain text (.txt)",
            Self::Json => "JSON (.json)",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::Txt => "TXT",
            Self::Json => "JSON",
        }
    }

    pub fn file_name(self, date: NaiveDate) -> String {
        format!("nexusvoice-transcripts-{date}.{}", self.extension())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportRow<'a> {
    id: i64,
    content: &'a str,
    created_at: &'a str,
    word_count: i64,
    #[serde(serialize_with = "js_number")]
    duration_seconds: Option<f64>,
}

/// Whole numbers without a fraction, as the old app's JS exports wrote them.
#[allow(clippy::ref_option)] // the signature `serialize_with` requires
fn js_number<S: Serializer>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
    match *value {
        #[allow(clippy::cast_possible_truncation)] // guarded by the fract/range check
        Some(v) if v.fract() == 0.0 && v.abs() < 1e15 => serializer.serialize_i64(v as i64),
        Some(v) => serializer.serialize_f64(v),
        None => serializer.serialize_none(),
    }
}

/// # Errors
/// Only if JSON serialization fails, which plain rows cannot cause.
pub fn export_content<Tz: TimeZone>(
    format: ExportFormat,
    items: &[TranscriptResponse],
    tz: &Tz,
) -> Result<String, serde_json::Error> {
    match format {
        ExportFormat::Txt => Ok(items
            .iter()
            .map(|t| format!("[{}]\n{}", fmt_date_in(&t.created_at, tz), t.content))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")),
        ExportFormat::Json => {
            let rows: Vec<_> = items
                .iter()
                .map(|t| ExportRow {
                    id: t.id,
                    content: &t.content,
                    created_at: &t.created_at,
                    word_count: t.word_count,
                    duration_seconds: t.duration_seconds,
                })
                .collect();
            serde_json::to_string_pretty(&rows)
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/ui/pages/dashboard/format.rs"]
mod tests;
