//! `DictionaryRepository` — the in-place update and the hand-built batch hit counter.
//! `increment_hits_batch` interpolates its own `VALUES` placeholders, so a
//! regression there is a silent SQL bug, not a type error.

use super::DictionaryRepository;
use crate::data::connection::init_database;
use crate::data::dto::dictionary::CreateDictionaryEntry;
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

fn new_entry(term: &str, replacement: &str) -> CreateDictionaryEntry {
    CreateDictionaryEntry {
        term: term.to_string(),
        replacement: replacement.to_string(),
    }
}

async fn hits(repo: &DictionaryRepository, term: &str) -> i64 {
    repo.list_all()
        .await
        .expect("list")
        .into_iter()
        .find(|e| e.term == term)
        .unwrap_or_else(|| panic!("{term} missing"))
        .hits
}

#[tokio::test]
async fn update_renames_in_place_keeping_the_id_and_hit_count() {
    // A rename must not reset how often the entry has fired, or give it a new id.
    let repo = DictionaryRepository::new(pool().await);
    let created = repo
        .create(new_entry("nexus", "Nexus"))
        .await
        .expect("insert");
    repo.increment_hits_batch(&["nexus".to_string()])
        .await
        .expect("bump");

    let updated = repo
        .update(created.id, new_entry("nexus voice", "NexusVoice"))
        .await
        .expect("update");

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.hits, 1);
    assert_eq!(updated.term, "nexus voice");
    assert_eq!(repo.list_all().await.expect("list").len(), 1);
}

#[tokio::test]
async fn increment_hits_batch_counts_repeats_within_one_transcript() {
    // One dictation can match the same term several times; each occurrence
    // counts, but the term is updated with a single row.
    let repo = DictionaryRepository::new(pool().await);
    repo.create(new_entry("api", "API")).await.expect("insert");
    repo.create(new_entry("json", "JSON"))
        .await
        .expect("insert");

    let matched = vec![
        "api".to_string(),
        "json".to_string(),
        "api".to_string(),
        "api".to_string(),
    ];
    repo.increment_hits_batch(&matched).await.expect("batch");

    assert_eq!(hits(&repo, "api").await, 3);
    assert_eq!(hits(&repo, "json").await, 1);
}

#[tokio::test]
async fn increment_hits_batch_accumulates_across_calls() {
    let repo = DictionaryRepository::new(pool().await);
    repo.create(new_entry("api", "API")).await.expect("insert");

    repo.increment_hits_batch(&["api".to_string()])
        .await
        .expect("one");
    repo.increment_hits_batch(&["api".to_string(), "api".to_string()])
        .await
        .expect("two");

    assert_eq!(hits(&repo, "api").await, 3);
}

#[tokio::test]
async fn increment_hits_batch_leaves_unmatched_terms_alone() {
    let repo = DictionaryRepository::new(pool().await);
    repo.create(new_entry("api", "API")).await.expect("insert");
    repo.create(new_entry("untouched", "Untouched"))
        .await
        .expect("insert");

    repo.increment_hits_batch(&["api".to_string()])
        .await
        .expect("batch");

    assert_eq!(hits(&repo, "api").await, 1);
    assert_eq!(hits(&repo, "untouched").await, 0);
}

#[tokio::test]
async fn increment_hits_batch_tolerates_a_term_not_in_the_dictionary() {
    // The corrector's matched list and the table can drift if an entry is
    // deleted mid-dictation; an unknown term must not fail the whole batch.
    let repo = DictionaryRepository::new(pool().await);
    repo.create(new_entry("api", "API")).await.expect("insert");

    repo.increment_hits_batch(&["api".to_string(), "deleted".to_string()])
        .await
        .expect("batch must not error on an unknown term");

    assert_eq!(hits(&repo, "api").await, 1);
}

#[tokio::test]
async fn increment_hits_batch_is_a_noop_for_an_empty_list() {
    let repo = DictionaryRepository::new(pool().await);
    repo.create(new_entry("api", "API")).await.expect("insert");

    repo.increment_hits_batch(&[]).await.expect("empty batch");

    assert_eq!(hits(&repo, "api").await, 0);
}

#[tokio::test]
async fn delete_reports_whether_anything_was_removed() {
    let repo = DictionaryRepository::new(pool().await);
    let entry = repo.create(new_entry("teh", "the")).await.expect("insert");

    assert!(repo.delete_by_id(entry.id).await.expect("delete"));
    assert!(
        !repo.delete_by_id(entry.id).await.expect("second delete"),
        "deleting a missing row reports false, not an error"
    );
    assert!(repo.list_all().await.expect("list").is_empty());
}
