//! The feed's page queries, keyset cursors and loaded rows.

use std::collections::HashSet;

use chrono::NaiveDate;

use crate::api::{dto::TranscriptResponse, PageQuery};

pub const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TranscriptFilters {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub sort_asc: bool,
}

impl TranscriptFilters {
    pub fn is_active(&self) -> bool {
        self.from.is_some() || self.to.is_some() || self.sort_asc
    }

    pub fn page_query(&self, cursor: Option<&Cursor>) -> PageQuery {
        PageQuery {
            #[allow(clippy::cast_possible_wrap)] // PAGE_SIZE is a small constant
            limit: Some(PAGE_SIZE as i64),
            cursor_created_at: cursor.map(|c| c.created_at.clone()),
            cursor_id: cursor.map(|c| c.id),
            from: self.from.map(|d| d.to_string()),
            to: self.to.map(|d| d.to_string()),
            sort_asc: Some(self.sort_asc),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub created_at: String,
    pub id: i64,
}

/// The rows loaded so far. The cursor is fixed per page, so deleting or
/// prepending a row never moves it.
#[derive(Debug, Default, PartialEq)]
pub struct Loaded {
    pub items: Vec<TranscriptResponse>,
    next: Option<Cursor>,
}

impl Loaded {
    pub fn from_first_page(rows: Vec<TranscriptResponse>) -> Self {
        let next = next_cursor(&rows);
        Self {
            items: dedup(rows),
            next,
        }
    }

    pub fn next_cursor(&self) -> Option<&Cursor> {
        self.next.as_ref()
    }

    pub fn has_more(&self) -> bool {
        self.next.is_some()
    }

    /// Add a following page, skipping rows an insert shifted across the boundary.
    pub fn append_page(&mut self, rows: Vec<TranscriptResponse>) {
        self.next = next_cursor(&rows);
        let seen: HashSet<i64> = self.items.iter().map(|t| t.id).collect();
        self.items
            .extend(dedup(rows).into_iter().filter(|t| !seen.contains(&t.id)));
    }

    /// Put a just-saved transcript on top unless loaded; returns whether it changed.
    pub fn prepend(&mut self, transcript: TranscriptResponse) -> bool {
        if self.items.iter().any(|t| t.id == transcript.id) {
            return false;
        }
        self.items.insert(0, transcript);
        true
    }

    pub fn remove(&mut self, id: i64) -> Option<usize> {
        let index = self.items.iter().position(|t| t.id == id)?;
        self.items.remove(index);
        Some(index)
    }
}

fn next_cursor(rows: &[TranscriptResponse]) -> Option<Cursor> {
    let last = rows.last().filter(|_| rows.len() >= PAGE_SIZE)?;
    Some(Cursor {
        created_at: last.created_at.clone(),
        id: last.id,
    })
}

fn dedup(rows: Vec<TranscriptResponse>) -> Vec<TranscriptResponse> {
    let mut seen = HashSet::new();
    rows.into_iter().filter(|t| seen.insert(t.id)).collect()
}

#[cfg(test)]
#[path = "../../../../tests/unit/ui/pages/dashboard/query.rs"]
mod tests;
