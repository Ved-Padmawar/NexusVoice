//! History written to a file in Rust, so its rows never cross into a window.

use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};

use super::repositories::transcript::{Filter, TranscriptRepository};
use super::rows::transcript::Transcript;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Txt,
    Json,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Json => "json",
        }
    }
}

/// Which rows go: what History showed when Export was clicked.
pub struct Selection {
    /// An FTS5 query; `None` lists every row.
    pub search: Option<String>,
    /// UTC `created_at` bounds.
    pub from: Option<String>,
    pub to: Option<String>,
    pub app: Option<String>,
    pub sort_desc: bool,
    /// Rows deleted behind an Undo the window still shows.
    pub exclude: HashSet<i64>,
}

/// One exported row, as the JSON format names it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Item<'a> {
    id: i64,
    content: &'a str,
    created_at: String,
    word_count: i64,
    duration_seconds: Option<f64>,
}

/// Every finished row `selection` matches and keeps, in one statement, so
/// the export is one consistent snapshot of the table.
async fn read(
    repo: &TranscriptRepository,
    selection: &Selection,
) -> Result<Vec<Transcript>, sqlx::Error> {
    // SQLite reads a negative LIMIT as none.
    const ALL: i64 = -1;
    let filter = Filter {
        from: selection.from.as_deref(),
        to: selection.to.as_deref(),
        app: selection.app.as_deref(),
    };
    let mut rows = match &selection.search {
        Some(query) => {
            repo.search(query, ALL, None, &filter, selection.sort_desc)
                .await?
        }
        None => {
            repo.list_keyset(ALL, None, &filter, selection.sort_desc)
                .await?
        }
    };
    rows.retain(|row| !row.pending && !selection.exclude.contains(&row.id));
    Ok(rows)
}

/// The `rows` as the file holds them.
fn render(format: ExportFormat, rows: &[Transcript]) -> Vec<u8> {
    match format {
        ExportFormat::Txt => rows
            .iter()
            .map(|row| {
                let local = Local.from_utc_datetime(&row.created_at);
                format!(
                    "[{}]
{}",
                    local.format("%b %-d, %I:%M %p"),
                    row.content
                )
            })
            .collect::<Vec<_>>()
            .join(
                "

---

",
            )
            .into_bytes(),
        ExportFormat::Json => {
            let items: Vec<Item> = rows
                .iter()
                .map(|row| Item {
                    id: row.id,
                    content: &row.content,
                    created_at: row.created_at.to_string(),
                    word_count: row.word_count,
                    duration_seconds: row.duration_seconds,
                })
                .collect();
            serde_json::to_vec_pretty(&items).expect("rows serialize")
        }
    }
}

/// Export `selection` into `dir` as
/// `nexusvoice-transcripts-<date>`, never over an existing file. The path and
/// row count; `None` when no row matched. A failed write leaves no file.
pub async fn export(
    repo: &TranscriptRepository,
    selection: Selection,
    dir: PathBuf,
    format: ExportFormat,
) -> Result<Option<(PathBuf, usize)>, String> {
    let rows = read(repo, &selection).await.map_err(|e| e.to_string())?;
    if rows.is_empty() {
        return Ok(None);
    }
    let file_name = format!(
        "nexusvoice-transcripts-{}.{}",
        Local::now().format("%Y-%m-%d"),
        format.extension()
    );
    // Off the runtime: a slow folder must not stall other commands.
    tauri::async_runtime::spawn_blocking(move || {
        let (path, mut file) = create_without_overwriting(&dir, &file_name)?;
        if let Err(e) = file.write_all(&render(format, &rows)) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        Ok(Some((path, rows.len())))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: std::io::Error| e.to_string())
}

/// Create `dir/file_name`, or the first free "name (n).ext". Creation is
/// exclusive, so an existing file is never clobbered.
fn create_without_overwriting(dir: &Path, file_name: &str) -> std::io::Result<(PathBuf, File)> {
    let name = Path::new(file_name);
    let stem = name.file_stem().unwrap_or_default().to_string_lossy();
    let extension = name
        .extension()
        .map_or_default(|e| format!(".{}", e.to_string_lossy()));
    for attempt in 1.. {
        let candidate = if attempt == 1 {
            dir.join(file_name)
        } else {
            dir.join(format!("{stem} ({attempt}){extension}"))
        };
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    unreachable!("the attempt counter is unbounded")
}

#[cfg(test)]
#[path = "../../tests/unit/data/export.rs"]
mod tests;
