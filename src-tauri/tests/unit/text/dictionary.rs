use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use super::{Dictionary, DictionaryError};
use crate::data::connection::init_database;
use crate::data::rows::dictionary::DictionaryEntry;

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    init_database(&pool).await.expect("migrations");
    pool
}

/// What the windows see, sorted by term.
fn listed(dictionary: &Dictionary) -> Vec<DictionaryEntry> {
    let mut entries: Vec<_> = dictionary
        .entries
        .read(|entries| entries.0.values().cloned().collect());
    entries.sort_unstable_by(|a, b| a.term.cmp(&b.term));
    entries
}

fn id_of(dictionary: &Dictionary, term: &str) -> i64 {
    listed(dictionary)
        .into_iter()
        .find(|e| e.term == term)
        .expect("listed")
        .id
}

#[tokio::test]
async fn blank_and_overlong_entries_are_rejected_and_others_trimmed() {
    let pool = pool().await;
    let dictionary = Dictionary::new();

    for (term, replacement) in [("  ", "Zoom"), ("zoom", " "), ("a b c d", "x")] {
        let result = dictionary.save(&pool, None, term, replacement).await;
        assert!(
            matches!(result, Err(DictionaryError::Invalid(_))),
            "{term:?}"
        );
    }
    dictionary
        .save(&pool, None, " zoom ", " Zoom ")
        .await
        .expect("save");
    let saved = &listed(&dictionary)[0];
    assert_eq!(
        (saved.term.as_str(), saved.replacement.as_str()),
        ("zoom", "Zoom")
    );
}

#[tokio::test]
async fn a_term_matching_another_entry_is_a_duplicate_but_a_rename_keeps_its_row() {
    let pool = pool().await;
    let dictionary = Dictionary::new();
    dictionary
        .save(&pool, None, "next js", "Next.js")
        .await
        .expect("first");
    let next = id_of(&dictionary, "next js");
    dictionary
        .save(&pool, None, "zoom", "Zoom")
        .await
        .expect("second");

    let Err(DictionaryError::Duplicate(message)) =
        dictionary.save(&pool, None, "NextJS", "Next.js").await
    else {
        panic!("a differently spaced and cased term is the same entry");
    };
    assert_eq!(message, "“NextJS” is already in your dictionary.");
    assert!(matches!(
        dictionary.save(&pool, Some(next), "ZOOM", "Zoom").await,
        Err(DictionaryError::Duplicate(_))
    ));

    dictionary
        .save(&pool, Some(next), "Next JS", "Next.js")
        .await
        .expect("rename");
    assert_eq!(id_of(&dictionary, "Next JS"), next);
    assert_eq!(listed(&dictionary).len(), 2);
}

#[tokio::test]
async fn a_saved_hit_is_counted_and_sent() {
    let pool = pool().await;
    let dictionary = Dictionary::new();
    let saved = dictionary
        .save(&pool, None, "teh", "the")
        .await
        .expect("save");

    assert_eq!(dictionary.correct(&pool, "teh cat").await, "the cat");

    assert_eq!(listed(&dictionary)[0].hits, 1);
    assert!(dictionary.entries.committed().rev > saved.rev);
}

#[tokio::test]
async fn a_hit_the_database_refused_is_neither_counted_nor_sent() {
    // Counted anyway, the window would show a use a restart takes back.
    let pool = pool().await;
    let dictionary = Dictionary::new();
    let saved = dictionary
        .save(&pool, None, "teh", "the")
        .await
        .expect("save");
    pool.close().await;

    assert_eq!(dictionary.correct(&pool, "teh cat").await, "the cat");

    assert_eq!(listed(&dictionary)[0].hits, 0);
    assert_eq!(dictionary.entries.committed(), saved);
}
